use crate::{buffer::Buffer, frame::Area, view::Direction};

mod build;
mod slot;
pub(super) use slot::{Glyph, Row, RowMetadata, ScrollOffset, Slot, SlotPosition};

/// Holds the grid of slots a view's text is laid out on, rebuilt whenever the buffer, the
/// area or the scroll changes.
///
/// `slots` always holds `area.len()` slots row by row, and `row_metadata` one entry per row,
/// `None` for the rows past the end of the buffer. The cursor fields and `row_start_byte`
/// only mean something during `rebuild`, where they track the next slot to write.
#[derive(Debug, Clone)]
pub(super) struct Layout {
    slots: Vec<Slot>,
    row_metadata: Vec<Option<RowMetadata>>,
    area: Area,
    /// Marks where the layout starts. While rebuilding, the row counts the rows actually
    /// skipped.
    scroll_offset: ScrollOffset,
    /// Counts the rows of the first line still to skip while rebuilding.
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
            row_metadata: Vec::with_capacity(area.height()),
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

    /// Returns where the layout starts. After a rebuild the row is clamped to the rows the first
    /// line actually has.
    #[must_use]
    pub(super) fn scroll_offset(&self) -> ScrollOffset {
        self.scroll_offset
    }

    /// Returns the scroll offset that puts `row` on top, none for a row past the end of the
    /// buffer.
    #[must_use]
    pub(super) fn offset_of_row(&self, row: usize) -> Option<ScrollOffset> {
        let line = self.row(row)?.metadata.line;
        // Lines only go down the rows, so the first row holding the line is where it starts
        let first = self
            .row_metadata
            .iter()
            .position(|metadata| metadata.is_some_and(|metadata| metadata.line == line))?;
        // The top line may be scrolled partway through already
        let skipped = if first == 0 {
            self.scroll_offset.row
        } else {
            0
        };
        Some(ScrollOffset::new(line, skipped + row - first))
    }

    /// Returns the last row that shows text, none when nothing fits.
    #[must_use]
    pub(super) fn last_row(&self) -> Option<usize> {
        self.row_metadata.iter().rposition(Option::is_some)
    }

    /// Returns the index of the last row `line` wraps onto when laid out `cols` wide.
    #[must_use]
    pub(super) fn last_row_of_line(buffer: &Buffer, cols: u16, line: usize) -> usize {
        // On a one row grid, skipping all of the line's rows clamps onto its last
        let area = Area::new(cols, 1);
        let scroll_offset = ScrollOffset::new(line, usize::MAX);
        let mut layout = Self::new(area, scroll_offset);
        layout.rebuild(buffer, area, scroll_offset);
        layout.scroll_offset().row
    }

    /// Returns the glyph a slot belongs to, following a tail to its glyph, none for markers
    /// and blanks.
    #[must_use]
    pub(super) fn glyph(&self, slot: Slot) -> Option<Glyph> {
        match slot {
            Slot::Glyph(glyph) => Some(glyph),
            Slot::Tail(head_index) => self.glyph(self.slot(head_index)?),
            Slot::Marker(_) | Slot::Blank => None,
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
        let start = self.slot_index(SlotPosition::new(0, index));
        let end = start + self.area.width();
        Some(Row {
            metadata: self.row_metadata.get(index).copied()??,
            slots: self.slots.get(start..end)?,
        })
    }

    #[must_use]
    pub(super) fn row_metadata(&self, index: usize) -> Option<RowMetadata> {
        self.row_metadata.get(index).copied()?
    }

    pub(super) fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        (0..self.area.height()).filter_map(|row| self.row(row))
    }

    pub(super) fn slots(&self) -> impl ExactSizeIterator<Item = (SlotPosition, Slot)> {
        self.slots.iter().copied().enumerate().map(|(index, slot)| {
            (
                SlotPosition::new(index % self.area.width(), index / self.area.width()),
                slot,
            )
        })
    }

    /// Returns the glyph in a slot, reading only within the row so a col past the edge can't
    /// reach into the next one.
    #[must_use]
    pub(super) fn glyph_at(&self, pos: SlotPosition) -> Option<Glyph> {
        let slot = self.row(pos.row)?.slots.get(pos.col).copied()?;
        self.glyph(slot)
    }

    /// Returns where the glyph holding `byte` starts, none if it is off screen.
    #[must_use]
    pub(super) fn find_byte(&self, byte: usize) -> Option<SlotPosition> {
        self.rows().enumerate().find_map(|(row, layout_row)| {
            Some(SlotPosition::new(
                self.column_of_byte(&layout_row, byte)?,
                row,
            ))
        })
    }

    /// Returns the column of the glyph holding `byte` in `row`, none if the row doesn't show
    /// the byte.
    fn column_of_byte(&self, row: &Row<'_>, byte: usize) -> Option<usize> {
        if !(row.metadata.from_byte..row.metadata.to_byte).contains(&byte) {
            return None;
        }
        // The first slot of a glyph is its head, tails come after it
        row.slots
            .iter()
            .position(|slot| self.slot_holds_byte(*slot, byte))
    }

    fn slot_holds_byte(&self, slot: Slot, byte: usize) -> bool {
        self.glyph(slot)
            .is_some_and(|glyph| glyph.to_range().contains(&byte))
    }

    /// Returns the next glyph from `pos` in a direction within the row, skipping the tails of the
    /// glyph at `pos`.
    #[must_use]
    pub(super) fn step(&self, pos: SlotPosition, direction: Direction) -> Option<Glyph> {
        let Row { metadata, slots } = self.row(pos.row)?;
        let glyph = |col: usize| self.glyph(*slots.get(col)?);
        let base = glyph(pos.col);
        let differs = |col: usize| glyph(col).filter(|found| Some(*found) != base);

        match direction {
            // Only up to the last used slot, the rest of the row is blank
            Direction::Forward => (pos.col + 1..metadata.len).find_map(differs),
            Direction::Backward => (0..pos.col.min(slots.len())).rev().find_map(differs),
        }
    }

    /// Returns the first (`Forward`) or last (`Backward`) glyph of a row.
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
        assert_eq!(
            layout.find_byte(4),
            Some(SlotPosition::new(2, 1)),
            "漢 starts after the ↩ marker"
        );
        assert!(
            matches!(layout.slot(4), Some(Slot::Blank)),
            "the gap is blank"
        );
        assert_eq!(
            layout
                .row(0)
                .map(|row| (row.metadata.to_byte, row.metadata.len)),
            Some((4, 4)),
            "the gap is not part of the row"
        );
        assert_eq!(
            layout.find_byte(7),
            Some(SlotPosition::new(4, 1)),
            "e follows 漢's tail"
        );
    }

    #[test]
    fn wrapped_rows() {
        // Width 4: "abcd" then "↩ ef"
        let layout = layout("abcdef", 4, 3);
        assert_eq!(
            layout.find_byte(4),
            Some(SlotPosition::new(2, 1)),
            "e starts after the ↩ marker"
        );
        assert_eq!(
            layout
                .edge_glyph(1, Direction::Forward)
                .map(|glyph| glyph.from_byte),
            Some(4),
            "the wrapped row starts on e"
        );
        assert_eq!(
            layout
                .edge_glyph(0, Direction::Backward)
                .map(|glyph| glyph.from_byte),
            Some(3),
            "the first row ends on d"
        );
    }

    #[test]
    fn skips_wrapped_rows() {
        // Width 4: "abcd" then "↩ ef", start on the second
        let layout = scrolled_layout("abcdef", 4, 3, ScrollOffset::new(0, 1));
        assert_eq!(
            layout.find_byte(4),
            Some(SlotPosition::new(2, 0)),
            "after the marker, on top"
        );
        assert_eq!(layout.find_byte(0), None, "the skipped row is off screen");
        assert_eq!(
            layout.scroll_offset(),
            ScrollOffset::new(0, 1),
            "one row was skipped"
        );
    }

    #[test]
    fn offset_of_row_counts_wrapped_rows() {
        // Width 4: "abcd", "↩ e\n", then "xy"
        let layout = layout("abcde\nxy", 4, 4);
        assert_eq!(
            layout.offset_of_row(1),
            Some(ScrollOffset::new(0, 1)),
            "line 0's wrapped row"
        );
        assert_eq!(
            layout.offset_of_row(2),
            Some(ScrollOffset::new(1, 0)),
            "line 1's first row"
        );
        assert_eq!(layout.offset_of_row(3), None, "past the end of the buffer");

        let layout = scrolled_layout("abcde\nxy", 4, 4, ScrollOffset::new(0, 1));
        assert_eq!(
            layout.offset_of_row(0),
            Some(ScrollOffset::new(0, 1)),
            "the top row keeps the rows already skipped"
        );
        assert_eq!(
            layout.offset_of_row(1),
            Some(ScrollOffset::new(1, 0)),
            "line 1 follows"
        );
        assert_eq!(
            layout.last_row(),
            Some(1),
            "line 1 is the last row with text"
        );
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
            layout.row(1).map(|row| row.metadata.line),
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
