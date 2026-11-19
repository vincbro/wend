use std::num::NonZeroUsize;

use crate::{buffer::Buffer, frame::Area, view::Direction};

mod build;
mod slot;
pub(super) use slot::{Glyph, Row, RowMetadata, ScrollOffset, Slot, SlotPosition};

/// Holds the grid of slots a view's text is laid out on, rebuilt whenever the buffer, the
/// area or the scroll changes.
///
/// `slots` always holds `area.len()` slots row by row, and `row_metadata` one entry per row,
/// `None` for the rows past the end of the buffer.
#[derive(Debug, Clone)]
pub(super) struct Wrap {
    slots: Vec<Slot>,
    row_metadata: Vec<Option<RowMetadata>>,
    area: Area,
    scroll_offset: ScrollOffset,
    /// The columns between tab stops.
    tab_width: usize,
}

impl Wrap {
    #[must_use]
    pub(super) fn new(area: Area, scroll_offset: ScrollOffset, tab_width: NonZeroUsize) -> Self {
        let mut wrap = Self {
            slots: Vec::with_capacity(area.len()),
            row_metadata: Vec::with_capacity(area.height()),
            area,
            scroll_offset,
            tab_width: tab_width.get(),
        };
        wrap.resize(area, scroll_offset);
        wrap
    }

    /// Returns where the wrap starts. After a rebuild the row is clamped to the rows the first
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
    /// Sets the columns between tab stops, from the next rebuild on.
    pub(super) fn set_tab_width(&mut self, tab_width: NonZeroUsize) {
        self.tab_width = tab_width.get();
    }

    #[must_use]
    pub(super) fn last_row(&self) -> Option<usize> {
        self.row_metadata.iter().rposition(Option::is_some)
    }

    /// Returns the index of the last row `line` wraps onto when laid out `cols` wide.
    ///
    /// Replaces the contents of this wrap with a one row grid, so it works as scratch
    /// space that is reused between calls.
    #[must_use]
    pub(super) fn last_row_of_line(&mut self, buffer: &Buffer, cols: u16, line: usize) -> usize {
        // On a one row grid, skipping all of the line's rows clamps onto its last
        self.rebuild(
            buffer,
            Area::new(cols, 1),
            ScrollOffset::new(line, usize::MAX),
        );
        self.scroll_offset().row
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
        self.rows().enumerate().find_map(|(row, wrap_row)| {
            Some(SlotPosition::new(
                self.column_of_byte(&wrap_row, byte)?,
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
    use crate::config::Config;

    fn tab_width() -> NonZeroUsize {
        Config::default().text.tab_width
    }

    fn wrap(text: &str, cols: u16, rows: u16) -> Wrap {
        scrolled_wrap(text, cols, rows, ScrollOffset::default())
    }

    fn scrolled_wrap(text: &str, cols: u16, rows: u16, scroll_offset: ScrollOffset) -> Wrap {
        let area = Area::new(cols, rows);
        let mut wrap = Wrap::new(area, scroll_offset, tab_width());
        wrap.rebuild(&Buffer::from(text), area, scroll_offset);
        wrap
    }

    #[test]
    fn step_skips_tails() {
        // The tab spans cols 0..4, 漢 spans 5..7
        let wrap = wrap("\ta漢b", 20, 2);
        let byte = |col: usize, direction: Direction| {
            wrap.step(SlotPosition::new(col, 0), direction)
                .map(Glyph::start)
        };

        assert_eq!(byte(0, Direction::Forward), Some(1), "past the tab");
        assert_eq!(byte(5, Direction::Forward), Some(5), "past 漢");
        assert_eq!(byte(7, Direction::Backward), Some(2), "onto 漢 from b");
        assert_eq!(byte(6, Direction::Backward), Some(1), "from 漢's tail");
        assert_eq!(byte(4, Direction::Backward), Some(0), "onto the tab");
        assert_eq!(
            wrap.find_byte(2),
            Some(SlotPosition::new(5, 0)),
            "a wide glyph is found at its head"
        );
    }

    #[test]
    fn wide_glyph_wraps_whole() {
        // Width 5: "abcd" leaves one col, too little for 漢, so it starts the next row
        let wrap = wrap("abcd漢e", 5, 3);
        assert_eq!(
            wrap.find_byte(4),
            Some(SlotPosition::new(2, 1)),
            "漢 starts after the ↩ marker"
        );
        assert!(
            matches!(wrap.slot(4), Some(Slot::Blank)),
            "the gap is blank"
        );
        assert_eq!(
            wrap.row(0)
                .map(|row| (row.metadata.to_byte, row.metadata.len)),
            Some((4, 4)),
            "the gap is not part of the row"
        );
        assert_eq!(
            wrap.find_byte(7),
            Some(SlotPosition::new(4, 1)),
            "e follows 漢's tail"
        );
    }

    /// Asserts that every tail follows its glyph on the same row.
    fn assert_tails_have_heads(wrap: &Wrap) {
        for (pos, slot) in wrap.slots() {
            let Slot::Tail(head_index) = slot else {
                continue;
            };
            let index = wrap.slot_index(pos);
            assert!(
                head_index < index && head_index / wrap.area.width() == index / wrap.area.width(),
                "the tail at {pos:?} points at {head_index}, off its row"
            );
            assert!(
                matches!(wrap.slot(head_index), Some(Slot::Glyph(_))),
                "the tail at {pos:?} points at {head_index}, which isn't a glyph"
            );
        }
    }

    #[test]
    fn a_glyph_wider_than_the_view_is_shown_as_a_replacement() {
        let wrap = wrap("a漢b", 1, 4);
        let Some(Slot::Glyph(glyph)) = wrap.slot(1) else {
            panic!("漢 is the second slot");
        };
        assert_eq!(glyph.tails(), 0, "it has no room for a tail");
        assert_eq!(
            glyph.grapheme().as_str_sanitize(),
            "\u{FFFD}",
            "shown as U+FFFD"
        );
        assert_eq!(glyph.to_range(), 1..4, "it still covers 漢's bytes");
        assert_tails_have_heads(&wrap);
    }

    #[test]
    fn a_tab_ends_at_the_end_of_its_row() {
        // Width 6: the tab would span cols 4..8, so it stops at 6, and a view scrolled to
        // the next row doesn't start on a tail
        let text = "abcd\txyz";
        for row in 0..2 {
            let wrap = scrolled_wrap(text, 6, 3, ScrollOffset::new(0, row));
            assert_tails_have_heads(&wrap);
        }
        let wrap = wrap(text, 6, 3);
        let Some(Slot::Glyph(tab)) = wrap.slot(4) else {
            panic!("the tab follows abcd");
        };
        assert_eq!(tab.tails(), 1, "up to the end of the row");
    }

    #[test]
    fn wrapped_rows() {
        // Width 4: "abcd" then "↩ ef"
        let wrap = wrap("abcdef", 4, 3);
        assert_eq!(
            wrap.find_byte(4),
            Some(SlotPosition::new(2, 1)),
            "e starts after the ↩ marker"
        );
        assert_eq!(
            wrap.edge_glyph(1, Direction::Forward).map(Glyph::start),
            Some(4),
            "the wrapped row starts on e"
        );
        assert_eq!(
            wrap.edge_glyph(0, Direction::Backward).map(Glyph::start),
            Some(3),
            "the first row ends on d"
        );
    }

    #[test]
    fn skips_wrapped_rows() {
        // Width 4: "abcd" then "↩ ef", start on the second
        let wrap = scrolled_wrap("abcdef", 4, 3, ScrollOffset::new(0, 1));
        assert_eq!(
            wrap.find_byte(4),
            Some(SlotPosition::new(2, 0)),
            "after the marker, on top"
        );
        assert_eq!(wrap.find_byte(0), None, "the skipped row is off screen");
        assert_eq!(
            wrap.scroll_offset(),
            ScrollOffset::new(0, 1),
            "one row was skipped"
        );
    }

    #[test]
    fn offset_of_row_counts_wrapped_rows() {
        // Width 4: "abcd", "↩ e\n", then "xy"
        let wrap = wrap("abcde\nxy", 4, 4);
        assert_eq!(
            wrap.offset_of_row(1),
            Some(ScrollOffset::new(0, 1)),
            "line 0's wrapped row"
        );
        assert_eq!(
            wrap.offset_of_row(2),
            Some(ScrollOffset::new(1, 0)),
            "line 1's first row"
        );
        assert_eq!(wrap.offset_of_row(3), None, "past the end of the buffer");

        let wrap = scrolled_wrap("abcde\nxy", 4, 4, ScrollOffset::new(0, 1));
        assert_eq!(
            wrap.offset_of_row(0),
            Some(ScrollOffset::new(0, 1)),
            "the top row keeps the rows already skipped"
        );
        assert_eq!(
            wrap.offset_of_row(1),
            Some(ScrollOffset::new(1, 0)),
            "line 1 follows"
        );
        assert_eq!(wrap.last_row(), Some(1), "line 1 is the last row with text");
    }

    #[test]
    fn last_row_of_line_counts_wrapped_rows() {
        // Width 4: line 0 is "abcd", "↩ e\n", line 1 is "xy"
        let buffer = Buffer::from("abcde\nxy");
        let mut scratch = Wrap::new(Area::new(0, 0), ScrollOffset::default(), tab_width());
        assert_eq!(scratch.last_row_of_line(&buffer, 4, 0), 1);
        assert_eq!(scratch.last_row_of_line(&buffer, 4, 1), 0);
    }

    #[test]
    fn last_row_of_line_leaves_nothing_from_the_last_call() {
        // Line 0 wraps three times at width 4, line 1 is short, line 2 wraps
        let buffer = Buffer::from(
            "abcdefghijkl
xy
abcdefg",
        );
        let mut scratch = Wrap::new(Area::new(0, 0), ScrollOffset::default(), tab_width());
        for (cols, line) in [(4, 0), (4, 1), (12, 0), (4, 2), (3, 0), (8, 2), (4, 1)] {
            let fresh = Wrap::new(Area::new(0, 0), ScrollOffset::default(), tab_width())
                .last_row_of_line(&buffer, cols, line);
            assert_eq!(
                scratch.last_row_of_line(&buffer, cols, line),
                fresh,
                "line {line} at width {cols} counts the same on reused scratch"
            );
        }
    }

    #[test]
    fn skip_clamps_to_the_last_row_of_the_line() {
        // Width 4: "abcd" then "↩ e\n", then "xy"
        let wrap = scrolled_wrap("abcde\nxy", 4, 3, ScrollOffset::new(0, usize::MAX));
        assert_eq!(
            wrap.scroll_offset(),
            ScrollOffset::new(0, 1),
            "line 0 only has two rows"
        );
        assert_eq!(
            wrap.find_byte(4),
            Some(SlotPosition::new(2, 0)),
            "its last row is on top"
        );
        assert_eq!(
            wrap.row(1).map(|row| row.metadata.line),
            Some(1),
            "the next line follows it"
        );

        // No line break, the end of buffer glyph wraps onto its own row and ends the line
        let wrap = scrolled_wrap("abcdef", 4, 3, ScrollOffset::new(0, usize::MAX));
        assert_eq!(
            wrap.scroll_offset(),
            ScrollOffset::new(0, 2),
            "the end of buffer row counts"
        );
        assert_eq!(
            wrap.find_byte(6),
            Some(SlotPosition::new(2, 0)),
            "the end of buffer glyph is on top"
        );
    }
}
