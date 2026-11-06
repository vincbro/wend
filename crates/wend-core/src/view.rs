use std::{mem, ops};

use crate::{
    buffer::{self, Buffer},
    frame::{self, Cell, CellPosition, Frame, Grapheme, Rect},
    syntax::Highlight,
};

mod edit;
mod highlight;
use highlight::Highlighter;
mod layout;
use layout::{Glyph, Layout, ScrollOffset, Slot, SlotPosition};
mod motion;
pub use motion::{Direction, Motion};
mod scroll;
mod selection;
pub use selection::Selection;
mod gutter;

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
    /// Scratch space for `render`, kept so that its memory is reused.
    open_highlights: Vec<(usize, Highlight)>,

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
            open_highlights: vec![],

            scroll_margin: 10,
            desired_col: 0,
        }
    }

    #[must_use]
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    #[must_use]
    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    /// Takes in the file's new contents. The selection keeps its bytes as far as the new
    /// text reaches, the same way a revert does.
    pub fn update(&mut self, bytes: &[u8]) -> buffer::Result<()> {
        self.buffer.update(bytes)?;
        self.layout_dirty = true;
        self.set_selection(self.selection, ColumnPolicy::Sync);
        self.check_invariants();
        Ok(())
    }

    /// Returns where the head is in the frame, scrolling first so it is on screen, none if
    /// the area is empty. The area comes from the terminal, which can report no size at all.
    pub fn cursor_position(&mut self) -> Option<CellPosition> {
        self.ensure_cursor_visible();
        let pos = self.head_position()?;
        Some(Self::cell_position(self.text_rect(), pos))
    }

    /// Returns the slot the head is in, none if it is off screen.
    fn head_position(&self) -> Option<SlotPosition> {
        self.layout.find_byte(self.selection.head())
    }

    /// Returns where a slot of the layout ends up in the frame.
    fn cell_position(rect: Rect, pos: SlotPosition) -> CellPosition {
        CellPosition::new(
            rect.col + frame::terminal_coordinate(pos.col),
            rect.row + frame::terminal_coordinate(pos.row),
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
        self.desired_col = self
            .desired_col
            .min(self.text_rect().area.width().saturating_sub(1));
        self.layout_dirty = true;
    }

    /// Returns how many columns the gutter takes from the left of the view. It grows with the
    /// largest number it has to show, which is a line number or a row distance, and takes at
    /// most half of the view so the text always has room.
    fn gutter_width(&self) -> u16 {
        // The cursor can sit on the empty line after a final line break, which the rope does
        // not count, so the last line number is one more
        let line_count = self.buffer.rope().line_len() + 1;
        let largest = line_count.max(self.rect.area.height());
        gutter::width(largest).min(self.rect.area.cols / 2)
    }

    /// Returns the part of the view the text is laid out in, to the right of the gutter.
    fn text_rect(&self) -> Rect {
        self.rect.split_left(self.gutter_width()).1
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

        if self.text_rect().area.is_empty() {
            return;
        }
        assert!(
            !self.layout_dirty && self.head_position().is_some(),
            "ensure_cursor_visible rebuilds the layout and scrolls the head on screen"
        );
        assert!(
            self.desired_col < self.text_rect().area.width(),
            "resize and rebuild_at clamp desired_col to the text width, got {}",
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
        let (gutter_rect, text_rect) = self.rect.split_left(self.gutter_width());
        let rows_with_text = self.layout.last_row().map_or(0, |row| row + 1);
        let head = self.head_position().and_then(|slot| {
            let metadata = self.layout.row_metadata(slot.row)?;
            Some(gutter::Head {
                row: slot.row,
                line: metadata.line,
            })
        });
        gutter::render(frame, gutter_rect, rows_with_text, head);

        let selected = self.selection_range();
        let mut open = mem::take(&mut self.open_highlights);
        // The highlights borrow the buffer for as long as the loop runs, so the loop only
        // reads the other fields, and doesn't call methods that take the whole view
        let spans = self.buffer.highlights(self.visible_bytes());
        let mut highlighter = Highlighter::new(spans, &mut open);
        for (pos, slot) in self.layout.slots() {
            let mut cell = Self::cell(&self.layout, self.selection, slot, pos, &selected);
            // Tails are blanks, only the glyph itself has a color
            if let Slot::Glyph(glyph) = slot {
                cell.highlight = highlighter.at(glyph.from_byte);
            }
            frame.write(Self::cell_position(text_rect, pos), cell);
        }

        self.open_highlights = open;
    }

    /// The bytes of the text the layout shows, from the first row's start to the last row's end.
    fn visible_bytes(&self) -> ops::Range<usize> {
        let mut rows = self.layout.rows();
        let Some(first) = rows.next() else {
            return 0..0;
        };
        let last = rows.last().unwrap_or(first);
        first.metadata.from_byte..last.metadata.to_byte
    }

    /// The cell the slot at `pos` renders as. `selected` is the selection's byte range.
    ///
    /// Takes the parts of the view it reads instead of the view, so `render` can keep the
    /// buffer borrowed meanwhile.
    fn cell(
        layout: &Layout,
        selection: Selection,
        slot: Slot,
        pos: SlotPosition,
        selected: &ops::Range<usize>,
    ) -> Cell {
        let blank = Cell {
            grapheme: Grapheme::blank(),
            ..Cell::default()
        };
        let Some(glyph) = layout.glyph(slot) else {
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
                let offset = layout.slot_index(pos) - head_index;
                (Grapheme::blank(), offset < glyph.grapheme.width())
            }
            _ => (glyph.grapheme, false),
        };

        Cell {
            grapheme,
            cursor: glyph.to_range().contains(&selection.head()),
            selected: selected.contains(&glyph.from_byte),
            covered,
            // Set by render, which walks the cells in order
            highlight: None,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        frame::Area,
        syntax::{self, LanguageTag},
    };

    /// A view whose text is `cols` columns wide, with the gutter on top of that.
    pub(crate) fn view(text: &str, cols: u16, rows: u16) -> View {
        // Wide enough that the gutter takes what it wants, whatever `cols` is
        let mut view = View::new(Rect::new(0, 0, Area::new(cols + 100, rows)));
        view.open_buffer(Buffer::from(text));
        let gutter = view.gutter_width();
        view.resize(Rect::new(0, 0, Area::new(cols + gutter, rows)));
        view.refresh_layout();
        view
    }

    /// Renders the view into a frame that is exactly as big as the view.
    fn rendered(view: &mut View) -> Frame {
        let mut frame = Frame::new(view.rect.area, None);
        view.render(&mut frame);
        frame
    }

    /// What the gutter shows on `row` of `frame`.
    fn gutter_text(view: &View, frame: &Frame, row: u16) -> String {
        frame.row(row)[..usize::from(view.gutter_width())]
            .iter()
            .map(|cell| cell.grapheme.as_str_sanitize())
            .collect()
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
    fn update_to_shorter_text_keeps_the_selection_inside() {
        let mut view = view("hello world", 20, 5);
        view.set_selection(Selection::new(6, 10), ColumnPolicy::Sync);
        view.update(b"hi").unwrap();

        assert_eq!(view.buffer.rope().to_string(), "hi");
        assert_eq!(
            (view.selection.anchor(), view.selection.head()),
            (2, 2),
            "both ends move back to the end of the new text"
        );
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
        let frame = rendered(&mut view);
        let gutter = view.gutter_width();
        let cell = |col: u16| frame.get(CellPosition::new(gutter + col, 0));

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
    fn render_colors_the_highlighted_text() {
        let (mut loader, grammar) = syntax::tests::rust();
        let mut view = view("fn main() {}", 20, 2);
        view.buffer_mut()
            .set_language(Some(LanguageTag::new("rust")));
        view.buffer_mut().attach(&grammar);
        loader.parse(view.buffer_mut()).unwrap();

        let frame = rendered(&mut view);

        let gutter = view.gutter_width();
        let highlight = |col: u16| frame.get(CellPosition::new(gutter + col, 0)).highlight;
        assert_eq!(highlight(0), Some(Highlight::Keyword), "the f of fn");
        assert_eq!(highlight(1), Some(Highlight::Keyword), "the n of fn");
        assert_eq!(highlight(2), None, "the space");
        assert_eq!(highlight(3), Some(Highlight::Function), "the m of main");
    }

    #[test]
    fn rows_are_numbered_by_their_distance_from_the_cursor() {
        let mut view = view(&"x\n".repeat(20), 20, 12);
        for _ in 0..5 {
            view.move_selection(Motion::Vertical(Direction::Forward), false);
        }

        let frame = rendered(&mut view);

        let shown: Vec<_> = (0..12).map(|row| gutter_text(&view, &frame, row)).collect();
        assert_eq!(
            shown,
            [
                "  5 ", "  4 ", "  3 ", "  2 ", "  1 ", "  6 ", "  1 ", "  2 ", "  3 ", "  4 ",
                "  5 ", "  6 "
            ],
            "the cursor is on row 5 of line 6, and the rest count rows away from it"
        );
    }

    #[test]
    fn wrapped_rows_count_like_any_other_row() {
        let mut view = view("aaaaaaaa\nb", 4, 8);

        let frame = rendered(&mut view);

        let rows_with_text = view.layout.last_row().unwrap() + 1;
        assert!(rows_with_text > 2, "the first line wraps");
        for row in 1..rows_with_text {
            assert_eq!(
                gutter_text(&view, &frame, u16::try_from(row).unwrap()),
                format!("{row:>3} "),
                "row {row} is {row} motions below the cursor, wrapped or not"
            );
        }
    }

    #[test]
    fn the_rows_below_the_end_of_the_file_have_no_number() {
        let mut view = view("a\nb", 20, 6);

        let frame = rendered(&mut view);

        assert_eq!(gutter_text(&view, &frame, 0), "  1 ", "the cursor's line");
        assert_eq!(gutter_text(&view, &frame, 1), "  1 ", "one row down");
        for row in 2..6 {
            assert_eq!(
                gutter_text(&view, &frame, row),
                "    ",
                "you can't move onto row {row}, so it has no number"
            );
        }
    }

    #[test]
    fn the_terminal_cursor_sits_in_the_text_and_not_the_gutter() {
        let mut view = view("abc", 20, 3);
        let gutter = view.gutter_width();

        assert_eq!(
            view.cursor_position(),
            Some(CellPosition::new(gutter, 0)),
            "right of the gutter"
        );
        view.move_selection(Motion::Horizontal(Direction::Forward), false);
        assert_eq!(
            view.cursor_position(),
            Some(CellPosition::new(gutter + 1, 0)),
            "one grapheme in"
        );
    }

    #[test]
    fn the_gutter_widens_past_999_lines_and_the_text_gives_up_a_column() {
        let mut view = view(&"a\n".repeat(998), 20, 5);
        assert_eq!(view.gutter_width(), 4, "999 lines fit three digits");
        assert_eq!(view.text_rect().area.cols, 20);

        view.new_line();

        assert_eq!(view.gutter_width(), 5, "1000 lines need a fourth digit");
        assert_eq!(view.text_rect().area.cols, 19, "the text gave up a column");
    }

    #[test]
    fn a_view_too_narrow_for_its_gutter_still_keeps_half_for_the_text() {
        let mut view = View::new(Rect::new(0, 0, Area::new(6, 3)));
        view.open_buffer(Buffer::from("abc"));

        assert_eq!(view.gutter_width(), 3, "half of 6, not the 4 it wants");
        assert_eq!(view.text_rect().area.cols, 3);
        assert!(view.cursor_position().is_some());
    }
}
