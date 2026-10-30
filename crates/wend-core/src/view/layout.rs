use std::ops;

use unicode_width::UnicodeWidthStr;

use crate::{area::Area, buffer::Buffer, view::Direction};

const TAB_WIDTH: usize = 4;

/// A grapheme as laid out: the bytes it covers, and how many tail slots follow its own
/// slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Glyph {
    pub from_byte: usize,
    pub to_byte: usize,
    pub tails: usize,
}

impl Glyph {
    #[must_use]
    pub(super) fn new(from_byte: usize, to_byte: usize, tails: usize) -> Self {
        Self {
            from_byte,
            to_byte,
            tails,
        }
    }

    #[must_use]
    pub(super) fn as_range(&self) -> ops::Range<usize> {
        self.from_byte..self.to_byte
    }
}

/// A slot in a view's layout grid. Relative to the view, not the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SlotPosition {
    pub col: usize,
    pub row: usize,
}

impl SlotPosition {
    #[must_use]
    pub(super) fn new(col: usize, row: usize) -> Self {
        Self { col, row }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Slot {
    Glyph(Glyph),
    /// The rest of a glyph wider than one slot. Holds the index of the glyph's own slot.
    Tail(usize),
    /// A marker that isn't text, such as the ↩ that starts a wrapped row.
    Misc(char),
    Blank,
}

/// The top of a view. The first line on screen and how many of its wrapped rows are
/// scrolled past, so the view can start partway through a line that wraps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct ScrollOffset {
    pub line: usize,
    pub row: usize,
}

impl ScrollOffset {
    #[must_use]
    pub(super) fn new(line: usize, row: usize) -> Self {
        Self { line, row }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Row<'a> {
    pub meta: RowMeta,
    pub slots: &'a [Slot],
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RowMeta {
    pub line: usize,
    pub from_byte: usize,
    pub to_byte: usize,
    /// How many slots the row uses, the rest of it is blank.
    pub len: usize,
}

/// The grid of slots a view's text is laid out on, rebuilt whenever the buffer, the area
/// or the scroll changes.
///
/// `slots` always holds `area.len()` slots row by row, and `row_meta` one entry per row,
/// `None` for the rows past the end of the buffer.
#[derive(Debug, Clone)]
pub(super) struct Layout {
    slots: Vec<Slot>,
    row_meta: Vec<Option<RowMeta>>,
    area: Area,
    /// Where the layout starts. While rebuilding, the row counts the rows actually
    /// skipped.
    scroll_offset: ScrollOffset,
    /// Rows of the first line still to skip while rebuilding.
    skip_rows: usize,

    row_cursor: usize,
    col_cursor: usize,
    line_cursor: usize,
    row_start_byte: usize,
    byte_cursor: usize,
}

impl Layout {
    #[must_use]
    pub(super) fn new(area: Area, scroll_offset: ScrollOffset) -> Self {
        let mut layout = Self {
            slots: Vec::with_capacity(area.len()),
            row_meta: Vec::with_capacity(area.height()),
            area,
            scroll_offset,
            skip_rows: 0,

            row_cursor: 0,
            col_cursor: 0,
            line_cursor: 0,

            row_start_byte: 0,
            byte_cursor: 0,
        };
        layout.resize(area, scroll_offset);
        layout
    }

    /// Where the layout starts. After a rebuild the row is clamped to the rows the first
    /// line actually has.
    #[must_use]
    pub(super) fn scroll_offset(&self) -> ScrollOffset {
        self.scroll_offset
    }

    /// The scroll offset that puts `row` on top, none for a row past the end of the
    /// buffer.
    #[must_use]
    pub(super) fn offset_of_row(&self, row: usize) -> Option<ScrollOffset> {
        let line = self.row(row)?.meta.line;
        // Lines only go down the rows, so the first row holding the line is where it starts
        let first = self
            .row_meta
            .iter()
            .position(|meta| meta.is_some_and(|meta| meta.line == line))?;
        // The top line may be scrolled partway through already
        let skipped = if first == 0 {
            self.scroll_offset.row
        } else {
            0
        };
        Some(ScrollOffset::new(line, skipped + row - first))
    }

    /// The last row that shows text, none when nothing fits.
    #[must_use]
    pub(super) fn last_row(&self) -> Option<usize> {
        self.row_meta.iter().rposition(Option::is_some)
    }

    /// The index of the last row `line` wraps onto when laid out `cols` wide.
    #[must_use]
    pub(super) fn last_row_of_line(buffer: &Buffer, cols: u16, line: usize) -> usize {
        // On a one row grid, skipping all of the line's rows clamps onto its last
        let area = Area::new(cols, 1);
        let scroll_offset = ScrollOffset::new(line, usize::MAX);
        let mut layout = Self::new(area, scroll_offset);
        layout.rebuild(buffer, area, scroll_offset);
        layout.scroll_offset().row
    }

    /// The glyph a slot belongs to, following a tail to its glyph. None for markers and
    /// blanks.
    #[must_use]
    pub(super) fn glyph(&self, slot: Slot) -> Option<Glyph> {
        match slot {
            Slot::Glyph(glyph) => Some(glyph),
            Slot::Tail(head_index) => self.glyph(self.slot(head_index)?),
            Slot::Misc(_) | Slot::Blank => None,
        }
    }

    #[must_use]
    pub(super) fn slot(&self, index: usize) -> Option<Slot> {
        self.slots.get(index).copied()
    }

    #[must_use]
    pub(super) fn slot_index(&self, pos: SlotPosition) -> usize {
        pos.col + pos.row * self.area.width()
    }

    #[must_use]
    pub(super) fn row(&self, index: usize) -> Option<Row<'_>> {
        let start = index * self.area.width();
        let end = start + self.area.width();
        Some(Row {
            meta: self.row_meta.get(index).copied()??,
            slots: self.slots.get(start..end)?,
        })
    }

    pub(super) fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        (0..self.area.height()).filter_map(|row| self.row(row))
    }

    /// The glyph in a slot, reading only within the row so a col past the edge can't
    /// reach into the next one.
    #[must_use]
    pub(super) fn glyph_at(&self, pos: SlotPosition) -> Option<Glyph> {
        let slot = self.row(pos.row)?.slots.get(pos.col).copied()?;
        self.glyph(slot)
    }

    /// Where the glyph holding `byte` starts, none if it is off screen.
    #[must_use]
    pub(super) fn find_byte(&self, byte: usize) -> Option<SlotPosition> {
        self.rows().enumerate().find_map(|(row, layout_row)| {
            if !(layout_row.meta.from_byte..layout_row.meta.to_byte).contains(&byte) {
                return None;
            }
            // The first slot of a glyph is its head, tails come after it
            let col = layout_row.slots.iter().position(|slot| {
                self.glyph(*slot)
                    .is_some_and(|glyph| glyph.as_range().contains(&byte))
            })?;
            Some(SlotPosition::new(col, row))
        })
    }

    /// The next glyph from `pos` in a direction within the row, skipping the tails of the
    /// glyph at `pos`.
    #[must_use]
    pub(super) fn step(&self, pos: SlotPosition, direction: Direction) -> Option<Glyph> {
        let Row { meta, slots } = self.row(pos.row)?;
        let glyph = |col: usize| self.glyph(*slots.get(col)?);
        let base = glyph(pos.col);
        let differs = |col: usize| glyph(col).filter(|found| Some(*found) != base);

        match direction {
            // Only up to the last used slot, the rest of the row is blank
            Direction::Forward => (pos.col + 1..meta.len).find_map(differs),
            Direction::Backward => (0..pos.col.min(slots.len())).rev().find_map(differs),
        }
    }

    /// The first (`Forward`) or last (`Backward`) glyph of a row.
    #[must_use]
    pub(super) fn edge_glyph(&self, row: usize, direction: Direction) -> Option<Glyph> {
        match direction {
            Direction::Forward => {
                let first = SlotPosition::new(0, row);
                self.glyph_at(first)
                    .or_else(|| self.step(first, Direction::Forward))
            }
            Direction::Backward => self.step(
                SlotPosition::new(self.area.width(), row),
                Direction::Backward,
            ),
        }
    }

    fn new_line(&mut self) {
        // The line ended before we skipped as many rows as asked, show its last row
        self.stop_skipping();
        self.seal();
        self.line_cursor += 1;
    }

    /// Seals the current row, appends row meta data.
    fn seal(&mut self) {
        // Past the last row, there is nothing left to seal
        if self.row_cursor >= self.area.height() {
            return;
        }

        // A skipped row is laid out like any other so it wraps the same, the next row
        // writes over its slots
        if self.skip_rows > 0 {
            self.skip_rows -= 1;
            let ScrollOffset { line, row } = self.scroll_offset;
            self.scroll_offset = ScrollOffset::new(line, row + 1);
            self.row_start_byte = self.byte_cursor;
            self.col_cursor = 0;
            return;
        }

        self.row_meta[self.row_cursor] = Some(RowMeta {
            line: self.line_cursor,
            from_byte: self.row_start_byte,
            to_byte: self.byte_cursor,
            len: self.col_cursor,
        });

        let from = self.slot_index(SlotPosition::new(self.col_cursor, self.row_cursor));
        let to = self.slot_index(SlotPosition::new(0, self.row_cursor + 1));
        self.slots
            .get_mut(from..to)
            .expect("the row cursor is below the area's height, so its row is inside the grid")
            .fill(Slot::Blank);

        self.row_start_byte = self.byte_cursor;

        self.col_cursor = 0;
        self.row_cursor += 1;
    }

    fn write_grapheme(&mut self, grapheme: &str) {
        let mut glyph = Glyph::new(
            self.byte_cursor,
            self.byte_cursor + grapheme.len(),
            grapheme.width().saturating_sub(1),
        );
        if grapheme == "\t" {
            // Up to the next tab stop, a whole tab width when already on one
            glyph.tails = TAB_WIDTH - self.col_cursor % TAB_WIDTH - 1;
            self.write(Slot::Glyph(glyph));
        } else {
            // A wide glyph that doesn't fit the rest of the row moves to the next one, the
            // terminal can't draw half of it. Unless it wouldn't fit there either.
            let cols = glyph.tails + 1;
            let fits_row = self.col_cursor + cols <= self.area.width();
            let fits_wrapped = self.wrap_col() + cols <= self.area.width();
            if !fits_row && fits_wrapped && self.col_cursor < self.area.width() {
                self.wrap();
            }
            self.write(Slot::Glyph(glyph));
            if grapheme.ends_with('\n') {
                self.new_line();
            }
        }
    }

    /// Writes a single slot, handles auto sealing and overflow protection.
    fn write(&mut self, slot: Slot) {
        // Wrap lazily, only once something actually needs the next row. Wrapping right
        // after filling a row would give a newline in the last column an empty ↩ row.
        if self.col_cursor >= self.area.width() {
            self.wrap();
        }

        if self.row_cursor >= self.area.height() {
            return;
        }

        let index = self.slot_index(SlotPosition::new(self.col_cursor, self.row_cursor));
        *self.slots.get_mut(index).expect(
            "both cursors are inside the area after the wrap and height checks, so the slot is \
             inside the grid",
        ) = slot;

        self.col_cursor += 1;

        if let Slot::Glyph(glyph) = slot {
            self.byte_cursor = glyph.to_byte;
            for _ in 0..glyph.tails {
                self.write(Slot::Tail(index));
            }
        }
    }

    fn stop_skipping(&mut self) {
        self.skip_rows = 0;
    }

    /// Seals the row and starts a wrapped one, its text begins at `wrap_col`.
    fn wrap(&mut self) {
        self.seal();
        if self.wrap_col() > 0 {
            self.write(Slot::Misc('↩'));
            self.write(Slot::Blank);
        }
    }

    /// The first col of a wrapped row free for text, after the markers.
    fn wrap_col(&self) -> usize {
        // The markers need room or they would wrap forever
        if self.area.width() > 2 { 2 } else { 0 }
    }

    fn resize(&mut self, area: Area, scroll_offset: ScrollOffset) {
        self.area = area;
        // Counted back up by the rows we actually skip
        self.scroll_offset = ScrollOffset::new(scroll_offset.line, 0);
        self.skip_rows = scroll_offset.row;

        self.row_meta.clear();
        self.row_meta.resize_with(self.area.height(), || None);
        self.slots.clear();
        self.slots.resize_with(area.len(), || Slot::Blank);

        self.col_cursor = 0;
        self.row_cursor = 0;
        self.line_cursor = scroll_offset.line;
    }

    pub(super) fn rebuild(&mut self, buffer: &Buffer, area: Area, scroll_offset: ScrollOffset) {
        self.resize(area, scroll_offset);
        // Nothing fits, and a row with no columns would never fill up
        if area.is_empty() {
            return;
        }

        let start = buffer.rope().byte_of_line(self.scroll_offset.line);
        self.byte_cursor = start;
        self.row_start_byte = start;

        let mut reached_end = true;
        for grapheme in buffer.rope().byte_slice(start..).graphemes() {
            if self.row_cursor >= self.area.height() {
                reached_end = false;
                break;
            }

            self.write_grapheme(&grapheme);

            // Here we will add linking logic for multiline slots
        }

        // A virtual glyph one past the last byte, so the cursor always has somewhere to be,
        // even in an empty buffer, and text can be appended at the end
        if reached_end {
            let end = buffer.rope().byte_len();
            self.write(Slot::Glyph(Glyph::new(end, end + 1, 0)));
        }

        // The buffer ended before we skipped as many rows as asked, show its last row
        self.stop_skipping();
        if self.col_cursor > 0 {
            self.seal();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(text: &str, cols: u16, rows: u16) -> Layout {
        scrolled_layout(text, cols, rows, ScrollOffset::default())
    }

    fn scrolled_layout(text: &str, cols: u16, rows: u16, scroll_offset: ScrollOffset) -> Layout {
        let area = Area::new(cols, rows);
        let mut layout = Layout::new(area, scroll_offset);
        layout.rebuild(&Buffer::from(text), area, scroll_offset);
        layout
    }

    #[test]
    fn step_skips_tails() {
        // The tab spans cols 0..4, 漢 spans 5..7
        let layout = layout("\ta漢b", 20, 2);
        let byte = |col: usize, direction: Direction| {
            layout
                .step(SlotPosition::new(col, 0), direction)
                .map(|glyph| glyph.from_byte)
        };

        assert_eq!(byte(0, Direction::Forward), Some(1), "past the tab");
        assert_eq!(byte(5, Direction::Forward), Some(5), "past 漢");
        assert_eq!(byte(7, Direction::Backward), Some(2), "onto 漢 from b");
        assert_eq!(byte(6, Direction::Backward), Some(1), "from 漢's tail");
        assert_eq!(byte(4, Direction::Backward), Some(0), "onto the tab");
        assert_eq!(
            layout.find_byte(2),
            Some(SlotPosition::new(5, 0)),
            "a wide glyph is found at its head"
        );
    }

    #[test]
    fn wide_glyph_wraps_whole() {
        // Width 5: "abcd" leaves one col, too little for 漢, so it starts the next row
        let layout = layout("abcd漢e", 5, 3);
        assert_eq!(layout.find_byte(4), Some(SlotPosition::new(2, 1)));
        assert!(
            matches!(layout.slot(4), Some(Slot::Blank)),
            "the gap is blank"
        );
        assert_eq!(
            layout.row(0).map(|row| (row.meta.to_byte, row.meta.len)),
            Some((4, 4)),
            "the gap is not part of the row"
        );
        assert_eq!(layout.find_byte(7), Some(SlotPosition::new(4, 1)));
    }

    #[test]
    fn wrapped_rows() {
        // Width 4: "abcd" then "↩ ef"
        let layout = layout("abcdef", 4, 3);
        assert_eq!(layout.find_byte(4), Some(SlotPosition::new(2, 1)));
        assert_eq!(
            layout
                .edge_glyph(1, Direction::Forward)
                .map(|glyph| glyph.from_byte),
            Some(4)
        );
        assert_eq!(
            layout
                .edge_glyph(0, Direction::Backward)
                .map(|glyph| glyph.from_byte),
            Some(3)
        );
    }

    #[test]
    fn skips_wrapped_rows() {
        // Width 4: "abcd" then "↩ ef", start on the second
        let layout = scrolled_layout("abcdef", 4, 3, ScrollOffset::new(0, 1));
        assert_eq!(layout.find_byte(4), Some(SlotPosition::new(2, 0)));
        assert_eq!(layout.find_byte(0), None, "the skipped row is off screen");
        assert_eq!(layout.scroll_offset(), ScrollOffset::new(0, 1));
    }

    #[test]
    fn offset_of_row_counts_wrapped_rows() {
        // Width 4: "abcd", "↩ e\n", then "xy"
        let layout = layout("abcde\nxy", 4, 4);
        assert_eq!(layout.offset_of_row(1), Some(ScrollOffset::new(0, 1)));
        assert_eq!(layout.offset_of_row(2), Some(ScrollOffset::new(1, 0)));
        assert_eq!(layout.offset_of_row(3), None, "past the end of the buffer");

        let layout = scrolled_layout("abcde\nxy", 4, 4, ScrollOffset::new(0, 1));
        assert_eq!(
            layout.offset_of_row(0),
            Some(ScrollOffset::new(0, 1)),
            "the top row keeps the rows already skipped"
        );
        assert_eq!(layout.offset_of_row(1), Some(ScrollOffset::new(1, 0)));
        assert_eq!(layout.last_row(), Some(1));
    }

    #[test]
    fn last_row_of_line_counts_wrapped_rows() {
        // Width 4: line 0 is "abcd", "↩ e\n", line 1 is "xy"
        let buffer = Buffer::from("abcde\nxy");
        assert_eq!(Layout::last_row_of_line(&buffer, 4, 0), 1);
        assert_eq!(Layout::last_row_of_line(&buffer, 4, 1), 0);
    }

    #[test]
    fn skip_clamps_to_the_last_row_of_the_line() {
        // Width 4: "abcd" then "↩ e\n", then "xy"
        let layout = scrolled_layout("abcde\nxy", 4, 3, ScrollOffset::new(0, usize::MAX));
        assert_eq!(
            layout.scroll_offset(),
            ScrollOffset::new(0, 1),
            "line 0 only has two rows"
        );
        assert_eq!(
            layout.find_byte(4),
            Some(SlotPosition::new(2, 0)),
            "its last row is on top"
        );
        assert_eq!(
            layout.row(1).map(|row| row.meta.line),
            Some(1),
            "the next line follows it"
        );

        // No line break, the end of buffer glyph wraps onto its own row and ends the line
        let layout = scrolled_layout("abcdef", 4, 3, ScrollOffset::new(0, usize::MAX));
        assert_eq!(
            layout.scroll_offset(),
            ScrollOffset::new(0, 2),
            "the end of buffer row counts"
        );
        assert_eq!(
            layout.find_byte(6),
            Some(SlotPosition::new(2, 0)),
            "the end of buffer glyph is on top"
        );
    }
}
