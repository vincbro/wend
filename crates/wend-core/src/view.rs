use std::{mem, ops};

use crate::{
    buffer::Buffer,
    frame::{self, Cell, CellPosition, Frame, Grapheme, Rect},
};

mod edit;
mod layout;
use layout::{Glyph, Layout, ScrollOffset, Slot, SlotPosition};
mod motion;
pub use motion::{Direction, Motion};
mod scroll;
mod selection;
pub use selection::Selection;

/// Decides what a selection change does to the column up and down aim for.
#[derive(Debug, Clone, Copy)]
enum ColumnPolicy {
    /// Keeps aiming for the same column, for moving up and down through short lines.
    Keep,
    /// Aims for the column the head is on now.
    Sync,
}

/// Shows a buffer in a rectangle of the frame, with a selection.
///
/// After every public call both ends of the selection sit on grapheme starts, the head
/// is on screen and `desired_col` is inside the width. `check_invariants` checks them.
#[derive(Debug, Clone)]
pub struct View {
    buffer: Buffer,
    rect: Rect,
    layout: Layout,
    layout_dirty: bool,
    selection: Selection,

    scroll_margin: usize,
    desired_col: usize,
}

impl View {
    #[must_use]
    pub fn new(rect: Rect) -> Self {
        Self {
            buffer: Buffer::new(),
            rect,
            layout: Layout::new(rect.area, ScrollOffset::default()),
            layout_dirty: true,
            selection: Selection::point(0),

            scroll_margin: 10,
            desired_col: 0,
        }
    }

    /// Returns where the head is in the frame, scrolling first so it is on screen, none if
    /// the area is empty. The area comes from the terminal, which can report no size at all.
    pub fn cursor_position(&mut self) -> Option<CellPosition> {
        self.ensure_cursor_visible();
        let pos = self.head_position()?;
        Some(self.cell_position(pos))
    }

    /// Returns the slot the head is in, none if it is off screen.
    fn head_position(&self) -> Option<SlotPosition> {
        self.layout.find_byte(self.selection.head())
    }

    /// Returns where a slot of the layout ends up in the frame.
    fn cell_position(&self, pos: SlotPosition) -> CellPosition {
        CellPosition::new(
            self.rect.col + frame::terminal_coordinate(pos.col),
            self.rect.row + frame::terminal_coordinate(pos.row),
        )
    }

    /// Returns the glyph holding `byte`, none if it is off screen or the layout is out of date.
    fn glyph_on_screen(&self, byte: usize) -> Option<Glyph> {
        if self.layout_dirty {
            return None;
        }
        self.layout.glyph_at(self.layout.find_byte(byte)?)
    }

    /// Returns where the grapheme holding `byte` starts, the buffer's end for anything past it.
    fn grapheme_start(&self, byte: usize) -> usize {
        self.glyph_on_screen(byte)
            .map_or_else(|| self.rope_grapheme_start(byte), |glyph| glyph.from_byte)
    }

    /// Walks the line from its start, so this costs as much as the line is long.
    fn rope_grapheme_start(&self, byte: usize) -> usize {
        let rope = self.buffer.rope();
        let mut byte = byte.min(rope.byte_len());
        // Line lookups need a char boundary, back up to one, at most 3 bytes in UTF-8
        while !rope.is_char_boundary(byte) {
            byte -= 1;
        }
        let mut start = rope.byte_of_line(rope.line_of_byte(byte));
        for grapheme in rope.byte_slice(start..).graphemes() {
            let end = start + grapheme.len();
            if end > byte {
                return start;
            }
            start = end;
        }
        start
    }

    /// Returns where the grapheme starting at `start` ends. The end of buffer slot is one byte
    /// wide, see `Layout::rebuild`.
    fn grapheme_end(&self, start: usize) -> usize {
        self.glyph_on_screen(start)
            .map_or_else(|| self.rope_grapheme_end(start), |glyph| glyph.to_byte)
    }

    /// Returns where the grapheme starting at `start` ends, inside the text even for the end
    /// of buffer slot.
    fn clamped_grapheme_end(&self, start: usize) -> usize {
        self.grapheme_end(start).min(self.buffer.rope().byte_len())
    }

    fn rope_grapheme_end(&self, start: usize) -> usize {
        let len = self
            .buffer
            .rope()
            .byte_slice(start..)
            .graphemes()
            .next()
            .map_or(1, |grapheme| grapheme.len());
        start + len
    }

    /// Returns the bytes the selection covers, from its first grapheme through its last.
    fn selection_range(&self) -> ops::Range<usize> {
        self.selection.start()..self.clamped_grapheme_end(self.selection.end())
    }

    /// The only way the selection changes. Snaps both ends to graphemes, scrolls so the
    /// head is visible and updates the column up and down aim for.
    fn set_selection(&mut self, selection: Selection, policy: ColumnPolicy) {
        self.selection = Selection::new(
            self.grapheme_start(selection.anchor()),
            self.grapheme_start(selection.head()),
        );
        self.ensure_cursor_visible();
        if let ColumnPolicy::Sync = policy {
            self.sync_desired_col();
        }
    }

    /// Makes the column the head is on the one up and down aim for.
    fn sync_desired_col(&mut self) {
        if let Some(pos) = self.head_position() {
            self.desired_col = pos.col;
        }
    }

    /// Returns where the text of `line` ends, before its line break.
    fn line_end(&self, line: usize) -> usize {
        let rope = self.buffer.rope();
        if line < rope.line_len() {
            rope.byte_of_line(line) + rope.line(line).byte_len()
        } else {
            rope.byte_len()
        }
    }

    pub fn resize(&mut self, rect: Rect) {
        self.rect = rect;
        self.desired_col = self.desired_col.min(rect.area.width().saturating_sub(1));
        self.layout_dirty = true;
    }

    /// Shows `new_buffer` from its start and returns the buffer shown until now.
    pub fn open_buffer(&mut self, mut new_buffer: Buffer) -> Buffer {
        mem::swap(&mut new_buffer, &mut self.buffer);
        // The old selection and scroll are byte and line offsets into the old buffer
        self.desired_col = 0;
        self.rebuild_at(ScrollOffset::default());
        self.set_selection(Selection::point(0), ColumnPolicy::Sync);
        new_buffer
    }

    /// Checks the rules every public function has to leave the view in. Debug builds only.
    fn check_invariants(&self) {
        if !cfg!(debug_assertions) {
            return;
        }

        let rope = self.buffer.rope();
        for byte in [self.selection.anchor(), self.selection.head()] {
            assert!(
                byte <= rope.byte_len() && rope.is_char_boundary(byte),
                "set_selection snaps both ends onto grapheme starts inside the rope, got {byte}"
            );
        }

        if self.rect.area.is_empty() {
            return;
        }
        assert!(
            !self.layout_dirty && self.head_position().is_some(),
            "ensure_cursor_visible rebuilds the layout and scrolls the head on screen"
        );
        assert!(
            self.desired_col < self.rect.area.width(),
            "resize clamps desired_col to the width, got {}",
            self.desired_col
        );
    }

    /// Writes the view's cells into `frame`, leaving the rest of it alone.
    ///
    /// # Panics
    ///
    /// If the view's rect reaches outside `frame`.
    pub fn render(&mut self, frame: &mut Frame) {
        self.refresh_layout();
        let selected = self.selection_range();
        for (pos, slot) in self.layout.slots() {
            frame.write(self.cell_position(pos), self.cell(slot, pos, &selected));
        }
    }

    /// The cell the slot at `pos` renders as. `selected` is the selection's byte range.
    fn cell(&self, slot: Slot, pos: SlotPosition, selected: &ops::Range<usize>) -> Cell {
        let blank = Cell {
            grapheme: Grapheme::blank(),
            ..Cell::default()
        };
        let Some(glyph) = self.layout.glyph(slot) else {
            return match slot {
                Slot::Marker(c) => Cell {
                    grapheme: Grapheme::from_char(c),
                    ..blank
                },
                _ => blank,
            };
        };

        // A tail is covered when the head prints wide enough to reach it, the rest (a tab,
        // a too long grapheme shown as U+FFFD) print as blanks
        let (grapheme, covered) = match slot {
            Slot::Tail(head_index) => {
                let offset = self.layout.slot_index(pos) - head_index;
                (Grapheme::blank(), offset < glyph.grapheme.width())
            }
            _ => (glyph.grapheme, false),
        };

        Cell {
            grapheme,
            cursor: glyph.to_range().contains(&self.selection.head()),
            selected: selected.contains(&glyph.from_byte),
            covered,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::frame::Area;

    pub(crate) fn view(text: &str, cols: u16, rows: u16) -> View {
        let mut view = View::new(Rect::new(0, 0, Area::new(cols, rows)));
        view.open_buffer(Buffer::from(text));
        view
    }

    #[test]
    fn set_selection_snaps_and_clamps() {
        // é is bytes 1..3
        let mut view = view("aé b", 20, 5);
        view.set_selection(Selection::point(2), ColumnPolicy::Sync);
        assert_eq!(
            view.selection,
            Selection::point(1),
            "snaps into the grapheme start"
        );

        view.set_selection(Selection::new(2, 100), ColumnPolicy::Sync);
        assert_eq!(
            view.selection,
            Selection::new(1, 5),
            "clamps to the end of the buffer"
        );
        view.check_invariants();
    }

    #[test]
    fn grapheme_bounds_from_the_layout_match_the_rope() {
        // Combining marks, wide glyphs, tabs, CRLF and wrapped rows, most of it off screen
        let text = "e\u{301}a漢\t🦀\r\nb a line long enough to wrap\n\n".repeat(4);
        let view = view(&text, 8, 6);
        for byte in 0..=text.len() + 1 {
            let start = view.grapheme_start(byte);
            assert_eq!(
                start,
                view.rope_grapheme_start(byte),
                "start of byte {byte}"
            );
            assert_eq!(
                view.grapheme_end(start),
                view.rope_grapheme_end(start),
                "end of the grapheme starting at {start}"
            );
        }
    }

    #[test]
    fn grapheme_start_ignores_an_out_of_date_layout() {
        let mut view = view("ab", 20, 5);
        // The mark joins a, the layout still has b on byte 1
        view.buffer.edit(1..1, "\u{301}");
        view.layout_dirty = true;
        assert_eq!(view.grapheme_start(1), 0, "reads the edited text");
    }

    #[test]
    fn render_covers_wide_tails_only() {
        let mut view = view("a😀b\tc", 20, 2);
        let mut frame = Frame::new(Area::new(20, 2), None);
        view.render(&mut frame);
        let cell = |col: u16| frame.get(CellPosition::new(col, 0));

        assert_eq!(cell(1).grapheme.as_str(), "😀");
        assert!(!cell(1).covered, "the head prints");
        assert!(cell(2).covered, "the emoji already draws its tail");
        assert_eq!(cell(3).grapheme.as_str(), "b");
        assert!(!cell(3).covered);
        // The tab spans cols 4..8, its head prints one blank so the tails must too
        assert!(
            (4..8).all(|col| !cell(col).covered),
            "tab tails print blanks"
        );
        assert_eq!(cell(8).grapheme.as_str(), "c");
    }
}
