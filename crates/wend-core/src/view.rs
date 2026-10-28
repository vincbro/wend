use std::{io, mem, ops};

use crate::{
    area::{Rect, saturating_u16},
    buffer::{Buffer, Change},
    frame::{Cell, CellPosition, Frame},
    grapheme::Grapheme,
};

pub mod layout;
use layout::{Layout, ScrollOffset, Slot, SlotPosition};
mod motion;
pub use motion::{Direction, Motion};
mod selection;
pub use selection::Selection;

/// What a selection change does to the column up and down aim for.
#[derive(Debug, Clone, Copy)]
enum ColumnPolicy {
    /// Keep aiming for the same column, for moving up and down through short lines.
    Keep,
    /// Aim for the column the head is on now.
    Sync,
}

#[derive(Debug, Clone)]
pub struct View {
    buffer: Buffer,
    rect: Rect,
    layout: Layout,
    layout_dirty: bool,
    selection: Selection,

    scroll_offset: ScrollOffset,
    scroll_marign: usize,
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

            scroll_offset: ScrollOffset::default(),
            scroll_marign: 10,
            desired_col: 0,
        }
    }

    /// Where the head is in the frame, scrolling first so it always is on screen.
    pub fn cursor_position(&mut self) -> CellPosition {
        self.ensure_cursor_visible();
        let pos = self
            .layout
            .find_byte(self.selection.head())
            .expect("The cursor is always visible after ensure_cursor_visible");
        self.cell_position(pos)
    }

    /// Where a slot of the layout ends up in the frame.
    fn cell_position(&self, pos: SlotPosition) -> CellPosition {
        CellPosition::new(
            self.rect.col.saturating_add(saturating_u16(pos.col)),
            self.rect.row.saturating_add(saturating_u16(pos.row)),
        )
    }

    /// Where the grapheme holding `byte` starts, the end of the buffer for anything past it.
    fn grapheme_start(&self, byte: usize) -> usize {
        let rope = self.buffer.rope();
        let mut byte = byte.min(rope.byte_len());
        // Line lookups need a char boundary, back up to one, at most 3 bytes in UTF-8
        while !rope.is_char_boundary(byte) {
            byte -= 1;
        }
        let mut start = rope.byte_of_line(rope.line_of_byte(byte));
        for grapheme in rope.byte_slice(start..).graphemes() {
            let end = start.saturating_add(grapheme.len());
            if end > byte {
                return start;
            }
            start = end;
        }
        start
    }

    /// Where the grapheme starting at `start` ends. The end of buffer slot is one byte
    /// wide, see `Layout::rebuild`.
    fn grapheme_end(&self, start: usize) -> usize {
        let len = self
            .buffer
            .rope()
            .byte_slice(start..)
            .graphemes()
            .next()
            .map_or(1, |grapheme| grapheme.len());
        start.saturating_add(len)
    }

    /// The bytes the selection covers, from its first grapheme through its last.
    fn selection_range(&self) -> ops::Range<usize> {
        let end = self
            .grapheme_end(self.selection.end())
            .min(self.buffer.rope().byte_len());
        self.selection.start()..end
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
        if let Some(pos) = self.layout.find_byte(self.selection.head()) {
            self.desired_col = pos.col;
        }
    }

    /// The only way the text changes. The selection follows the edit.
    fn edit(&mut self, range: ops::Range<usize>, text: &str) -> Change {
        let change = self.buffer.edit(range, text);
        self.layout_dirty = true;
        self.set_selection(self.selection.map(change), ColumnPolicy::Sync);
        change
    }

    /// Inserts in front of the selection.
    pub fn insert(&mut self, key: char) {
        let mut buf = [0; 4];
        let text = key.encode_utf8(&mut buf);
        let at = self.selection.start();
        self.edit(at..at, text);
        self.check_invariants();
    }

    /// Deletes the grapheme before the selection, joins lines when at the start of one.
    pub fn delete(&mut self) {
        // Done on the buffer, not the layout, so it works across wrapped rows and lines
        let to_byte = self.selection.start();
        let Some(len) = self
            .buffer
            .rope()
            .byte_slice(..to_byte)
            .graphemes()
            .next_back()
            .map(|grapheme| grapheme.len())
        else {
            return;
        };
        self.edit(to_byte.saturating_sub(len)..to_byte, "");
        self.check_invariants();
    }

    /// Deletes everything the selection covers and collapses onto what follows.
    pub fn delete_selection(&mut self) {
        let range = self.selection_range();
        if range.is_empty() {
            return;
        }
        // Both ends are inside the deleted range, so they map onto its start
        self.edit(range, "");
        self.check_invariants();
    }

    /// Opens a new line below the head's line and puts the cursor on it.
    pub fn new_line(&mut self) {
        let rope = self.buffer.rope();
        let line = rope.line_of_byte(self.selection.head());
        // The end of the line's text, before its line break
        let line_end = if line < rope.line_len() {
            rope.byte_of_line(line)
                .saturating_add(rope.line(line).byte_len())
        } else {
            rope.byte_len()
        };

        self.edit(line_end..line_end, "\n");
        self.set_selection(
            Selection::point(line_end.saturating_add(1)),
            ColumnPolicy::Sync,
        );
        self.check_invariants();
    }

    /// Throws away every edit since the file was last loaded or saved.
    pub fn revert(&mut self) {
        self.buffer.revert();
        self.layout_dirty = true;
        // There is no change to map through, keep the same bytes as close as the text allows
        self.set_selection(self.selection, ColumnPolicy::Sync);
        self.check_invariants();
    }

    pub fn save(&mut self) -> io::Result<()> {
        self.buffer.save()
    }

    fn ensure_cursor_visible(&mut self) {
        let height = self.rect.area.height();
        let margin = self.scroll_marign.min(height.saturating_sub(1) / 2);

        // The buffer or area might have changed since the last rebuild
        self.refresh_layout();

        let head = self.selection.head();
        if self.layout.find_byte(head).is_none() {
            // Put the head's line on top, then a screen at a time through it until the head
            // shows up. A jump never passes the head, the margin pulls it into place below.
            self.scroll_offset = ScrollOffset::new(self.buffer.rope().line_of_byte(head), 0);
            self.rebuild();
            while self.layout.find_byte(head).is_none() {
                let prev_offset = self.scroll_offset;
                self.scroll_offset.row = self.scroll_offset.row.saturating_add(height.max(1));
                self.rebuild();
                if self.scroll_offset == prev_offset {
                    break;
                }
            }
        }

        // One row at a time, so the head moves one row per step and stops right at the
        // margin. The margin is at most half the screen, the head can't be in both.
        while let Some(SlotPosition { row, .. }) = self.layout.find_byte(head) {
            let direction = if row < margin {
                Direction::Backward
            } else if row + margin >= height {
                Direction::Forward
            } else {
                break;
            };
            if !self.scroll_row(direction) {
                break;
            }
        }
    }

    /// Scrolls one row, false if there is nowhere further to go.
    fn scroll_row(&mut self, direction: Direction) -> bool {
        let prev_offset = self.scroll_offset;
        let ScrollOffset { line, row } = prev_offset;
        match direction {
            Direction::Forward => {
                self.scroll_offset = ScrollOffset::new(line, row.saturating_add(1));
                self.rebuild();
                // Clamped back, we were on the line's last row, start the next line
                if self.scroll_offset == prev_offset {
                    self.scroll_offset = ScrollOffset::new(line.saturating_add(1), 0);
                    self.rebuild();
                }
            }
            Direction::Backward => {
                self.scroll_offset = match row.checked_sub(1) {
                    Some(row) => ScrollOffset::new(line, row),
                    // The layout clamps it onto the last row of the line above
                    None if line > 0 => ScrollOffset::new(line - 1, usize::MAX),
                    None => return false,
                };
                self.rebuild();
            }
        }
        self.scroll_offset != prev_offset
    }

    /// Rebuilds the layout if anything it depends on changed since the last rebuild.
    fn refresh_layout(&mut self) {
        if self.layout_dirty {
            self.rebuild();
        }
    }

    fn rebuild(&mut self) {
        // Edits and reverts can make the buffer shorter than where we scrolled to, the
        // layout clamps the row the same way
        self.scroll_offset.line = self.scroll_offset.line.min(self.buffer.rope().line_len());
        self.layout
            .rebuild(&self.buffer, self.rect.area, self.scroll_offset);
        self.scroll_offset = self.layout.scroll_offset();
        self.layout_dirty = false;
    }

    pub fn resize(&mut self, rect: Rect) {
        self.rect = rect;
        self.desired_col = self.desired_col.min(rect.area.width().saturating_sub(1));
        self.layout_dirty = true;
    }

    pub fn open_buffer(&mut self, mut new_buffer: Buffer) -> Buffer {
        mem::swap(&mut new_buffer, &mut self.buffer);
        // The old selection and scroll are byte and line offsets into the old buffer
        self.scroll_offset = ScrollOffset::default();
        self.desired_col = 0;
        self.layout_dirty = true;
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
                "Selection end {byte} is not on a char boundary"
            );
        }

        if self.rect.area.is_empty() {
            return;
        }
        assert!(
            !self.layout_dirty && self.layout.find_byte(self.selection.head()).is_some(),
            "The head is not on screen"
        );
        assert!(
            self.desired_col < self.rect.area.width(),
            "desired_col {} is past the edge",
            self.desired_col
        );
    }

    pub fn render(&mut self, frame: &mut Frame) {
        self.refresh_layout();
        let head = self.selection.head();
        let selected = self.selection_range();

        for (row, line) in self.layout.rows().enumerate() {
            for (col, slot) in line.slots.iter().copied().enumerate() {
                let cell = if let Some(glyph) = self.layout.glyph(slot) {
                    // The end of buffer slot points past the rope, it has nothing to show
                    let grapheme = if glyph.from_byte >= self.buffer.rope().byte_len() {
                        Grapheme::from_char(' ')
                    } else {
                        self.buffer.get_grapheme(glyph.as_range())
                    };

                    // A tail is covered when the head prints wide enough to reach it, the
                    // rest (a tab, a too long grapheme shown as U+FFFD) print as blanks
                    let (grapheme, covered) = match slot {
                        Slot::Tail(head_idx) => {
                            let offset = self
                                .layout
                                .slot_index(SlotPosition::new(col, row))
                                .saturating_sub(head_idx);
                            (Grapheme::from_char(' '), offset < grapheme.width())
                        }
                        _ => (grapheme, false),
                    };

                    Cell {
                        grapheme,
                        cursor: glyph.as_range().contains(&head),
                        selected: selected.contains(&glyph.from_byte),
                        covered,
                        z_index: 0,
                    }
                } else if let Slot::Misc(char) = slot {
                    Cell {
                        grapheme: Grapheme::from_char(char),
                        cursor: false,
                        selected: false,
                        covered: false,
                        z_index: 0,
                    }
                } else {
                    Cell {
                        grapheme: Grapheme::from_char(' '),
                        cursor: false,
                        selected: false,
                        covered: false,
                        z_index: 0,
                    }
                };
                frame.write(self.cell_position(SlotPosition::new(col, row)), cell);
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::area::Area;

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
    fn delete_selection_collapses_onto_start() {
        let mut view = view("hello world", 20, 5);
        view.set_selection(Selection::new(0, 4), ColumnPolicy::Sync);
        view.delete_selection();
        assert_eq!(view.buffer.rope().to_string(), " world");
        assert_eq!(view.selection, Selection::point(0));
    }

    #[test]
    fn delete_selection_handles_wide_graphemes() {
        let mut view = view("a😀b", 20, 5);
        view.set_selection(Selection::point(1), ColumnPolicy::Sync);
        view.delete_selection();
        assert_eq!(view.buffer.rope().to_string(), "ab");
        assert_eq!(view.selection, Selection::point(1));
    }

    #[test]
    fn render_covers_wide_tails_only() {
        let mut view = view("a😀b\tc", 20, 2);
        let mut frame = Frame::new(Area::new(20, 2));
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

    #[test]
    fn scrolls_through_a_wrapped_line_a_row_at_a_time() {
        // Width 4, one line: "aaaa" then rows of "↩ aa". Height 5 gives a margin of 2.
        let mut view = view(&"a".repeat(40), 4, 5);
        let mut rows = vec![];
        for _ in 0..12 {
            view.move_selection(Motion::Vertical(Direction::Forward), false);
            let pos = view.layout.find_byte(view.selection.head()).unwrap();
            rows.push((pos.row, view.scroll_offset));
        }
        // Down to the margin, then the head stays on it and the view follows row by row
        let expected: Vec<_> = (1..=12)
            .map(|step: usize| {
                let row = step.min(2);
                (row, ScrollOffset::new(0, step - row))
            })
            .collect();
        assert_eq!(rows, expected);

        // And back up, the head stays on the top margin until the view reaches the top
        view.move_selection(Motion::Vertical(Direction::Backward), false);
        view.move_selection(Motion::Vertical(Direction::Backward), false);
        view.move_selection(Motion::Vertical(Direction::Backward), false);
        let pos = view.layout.find_byte(view.selection.head()).unwrap();
        assert_eq!((pos.row, view.scroll_offset), (2, ScrollOffset::new(0, 7)));
    }

    #[test]
    fn scrolls_up_onto_the_last_row_of_the_line_above() {
        // Width 4: line 0 is "aaaa", "↩ a\n", line 1 is "b"
        let mut view = view("aaaaa\nb\nc\nd", 4, 3);
        view.scroll_offset = ScrollOffset::new(1, 0);
        view.rebuild();
        assert!(view.scroll_row(Direction::Backward));
        assert_eq!(view.scroll_offset, ScrollOffset::new(0, 1));
        assert!(view.scroll_row(Direction::Backward));
        assert_eq!(view.scroll_offset, ScrollOffset::new(0, 0));
        assert!(!view.scroll_row(Direction::Backward), "already at the top");
    }

    #[test]
    fn jumps_into_the_middle_of_a_tall_line() {
        // One line far taller than the screen, the end of it can still be reached
        let mut view = view(&"a".repeat(200), 4, 5);
        view.move_selection(Motion::File(Direction::Forward), false);
        assert_eq!(view.selection.head(), 199, "the head is not pulled back");
        let pos = view.layout.find_byte(view.selection.head()).unwrap();
        assert_eq!(pos.row, 2, "on the bottom margin");
    }
}
