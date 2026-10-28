use crate::{area::Area, buffer::Buffer, view::Direction};

use std::ops;

use unicode_width::UnicodeWidthStr;

const TAB_WIDTH: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Glyph {
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
    Tail(usize),
    Misc(char),
    Blank,
}

impl Slot {
    #[must_use]
    pub(super) const fn is_glyph(self) -> bool {
        matches!(self, Self::Glyph(_))
    }

    #[must_use]
    pub(super) const fn is_tail(self) -> bool {
        matches!(self, Self::Tail { .. })
    }

    #[must_use]
    pub(super) const fn is_misc(self) -> bool {
        matches!(self, Self::Misc(_))
    }

    #[must_use]
    pub(super) const fn is_blank(self) -> bool {
        matches!(self, Self::Blank)
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
    // How many slots unitl the end of the line
    pub len: usize,
}

#[derive(Debug, Clone)]
pub(super) struct Layout {
    slots: Vec<Slot>,
    row_meta: Vec<Option<RowMeta>>,
    area: Area,
    // From which line we start from
    scroll_offset: usize,

    row_ptr: usize,
    col_ptr: usize,
    line_ptr: usize,
    row_byte_ptr: usize,
    byte_ptr: usize,
}

impl Layout {
    #[must_use]
    pub(super) fn new(area: Area, scroll_offset: usize) -> Self {
        let mut layout = Self {
            slots: Vec::with_capacity(area.len()),
            row_meta: Vec::with_capacity(area.height()),
            area,
            scroll_offset,

            row_ptr: 0,
            col_ptr: 0,
            line_ptr: 0,

            row_byte_ptr: 0,
            byte_ptr: 0,
        };
        layout.resize(area, scroll_offset);
        layout
    }

    // Gets the glyph, none if it's Blank
    #[must_use]
    pub(super) fn glyph(&self, slot: Slot) -> Option<Glyph> {
        match slot {
            Slot::Glyph(glyph) => Some(glyph),
            Slot::Tail(slot_idx) => self.glyph(self.slot(slot_idx)?),
            Slot::Misc(_) | Slot::Blank => None,
        }
    }

    #[must_use]
    pub(super) fn slot(&self, slot_idx: usize) -> Option<Slot> {
        self.slots.get(slot_idx).copied()
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
        (0..self.area.height()).filter_map(|idx| self.row(idx))
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
        self.rows().enumerate().find_map(|(row_idx, row)| {
            if !(row.meta.from_byte..row.meta.to_byte).contains(&byte) {
                return None;
            }
            // The first slot of a glyph is its head, tails come after it
            let col_idx = row.slots.iter().position(|slot| {
                self.glyph(*slot)
                    .is_some_and(|glyph| glyph.as_range().contains(&byte))
            })?;
            Some(SlotPosition::new(col_idx, row_idx))
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
            Direction::Forward => (pos.col.saturating_add(1)..meta.len).find_map(differs),
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

    /// The last glyph on screen.
    #[must_use]
    pub(super) fn last_glyph(&self) -> Option<Glyph> {
        (0..self.area.height())
            .rev()
            .find_map(|row| self.edge_glyph(row, Direction::Backward))
    }

    fn new_line(&mut self) {
        self.seal();
        self.line_ptr = self.line_ptr.saturating_add(1);
    }

    /// Seals the current row, appends row meta data.
    fn seal(&mut self) {
        // Past the last row, there is nothing left to seal
        if self.row_ptr >= self.area.height() {
            return;
        }

        self.row_meta[self.row_ptr] = Some(RowMeta {
            line: self.line_ptr,
            from_byte: self.row_byte_ptr,
            to_byte: self.byte_ptr,
            len: self.col_ptr,
        });

        for _ in self.col_ptr..self.area.width() {
            let idx = self.col_ptr + self.row_ptr * self.area.width();
            let Some(slot_mut) = self.slots.get_mut(idx) else {
                panic!("Should not be able to overflow");
            };
            *slot_mut = Slot::Blank;
            self.col_ptr = self.col_ptr.saturating_add(1);
        }

        self.row_byte_ptr = self.byte_ptr;

        self.col_ptr = 0;
        self.row_ptr = self.row_ptr.saturating_add(1);
    }

    fn write_grapheme(&mut self, grapheme: &str) {
        let mut glyph = Glyph::new(
            self.byte_ptr,
            self.byte_ptr.saturating_add(grapheme.len()),
            grapheme.width().saturating_sub(1),
        );
        if grapheme == "\t" {
            let jumps = if self.col_ptr.is_multiple_of(TAB_WIDTH) {
                TAB_WIDTH
            } else {
                TAB_WIDTH - (self.col_ptr % TAB_WIDTH)
            }
            .saturating_sub(1);
            glyph.tails = jumps;
            self.write(Slot::Glyph(glyph));
        } else {
            // A wide glyph that doesn't fit the rest of the row moves to the next one, the
            // terminal can't draw half of it. Unless it wouldn't fit there either.
            let cols = glyph.tails.saturating_add(1);
            let fits_row = self.col_ptr.saturating_add(cols) <= self.area.width();
            let fits_wrapped = self.wrap_col().saturating_add(cols) <= self.area.width();
            if !fits_row && fits_wrapped && self.col_ptr < self.area.width() {
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
        if self.col_ptr >= self.area.width() {
            self.wrap();
        }

        if self.row_ptr >= self.area.height() {
            return;
        }

        let idx = self.slot_index(SlotPosition::new(self.col_ptr, self.row_ptr));
        let Some(slot_mut) = self.slots.get_mut(idx) else {
            panic!("Should not be able to overflow");
        };
        *slot_mut = slot;

        self.col_ptr = self.col_ptr.saturating_add(1);

        if let Slot::Glyph(glyph) = slot {
            self.byte_ptr = glyph.to_byte;
            for _ in 0..glyph.tails {
                self.write(Slot::Tail(idx));
            }
        }
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

    fn resize(&mut self, area: Area, scroll_offset: usize) {
        self.area = area;
        self.scroll_offset = scroll_offset;

        self.row_meta.clear();
        self.row_meta.resize_with(self.area.height(), || None);
        self.slots.clear();
        self.slots.resize_with(area.len(), || Slot::Blank);

        self.col_ptr = 0;
        self.row_ptr = 0;
        self.line_ptr = scroll_offset;
    }

    pub(super) fn rebuild(&mut self, buffer: &Buffer, area: Area, scroll_offset: usize) {
        self.resize(area, scroll_offset);

        let start = buffer.rope().byte_of_line(self.scroll_offset);
        self.byte_ptr = start;
        self.row_byte_ptr = start;

        let mut reached_end = true;
        for grapheme in buffer.rope().byte_slice(start..).graphemes() {
            if self.row_ptr >= self.area.height() {
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
            self.write(Slot::Glyph(Glyph::new(end, end.saturating_add(1), 0)));
        }

        if self.col_ptr > 0 {
            self.seal();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(text: &str, cols: u16, rows: u16) -> Layout {
        let area = Area::new(cols, rows);
        let mut layout = Layout::new(area, 0);
        layout.rebuild(&Buffer::from(text), area, 0);
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
            layout.slot(4).is_some_and(Slot::is_blank),
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
}
