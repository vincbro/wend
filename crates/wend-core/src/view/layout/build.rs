use unicode_width::UnicodeWidthStr;

use super::{Glyph, Layout, RowMetadata, ScrollOffset, Slot, SlotPosition};
use crate::{
    buffer::Buffer,
    frame::{Area, Grapheme},
};

const TAB_WIDTH: usize = 4;

impl Layout {
    fn new_line(&mut self) {
        // The line ended before we skipped as many rows as asked, show its last row
        self.skip_rows = 0;
        self.seal();
        self.line_cursor += 1;
    }

    /// Seals the current row, appends row metadata.
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

        self.row_metadata[self.row_cursor] = Some(RowMetadata {
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
        if grapheme == "\t" {
            // Up to the next tab stop, a whole tab width when already on one
            let tails = TAB_WIDTH - self.col_cursor % TAB_WIDTH - 1;
            self.write(Slot::Glyph(self.glyph_at_cursor(grapheme, tails)));
            return;
        }

        // A wide glyph that doesn't fit the rest of the row moves to the next one, the
        // terminal can't draw half of it. Unless it wouldn't fit there either.
        let tails = grapheme.width().saturating_sub(1);
        let cols = tails + 1;
        let fits_row = self.col_cursor + cols <= self.area.width();
        let fits_wrapped = self.wrap_col() + cols <= self.area.width();
        if !fits_row && fits_wrapped && self.col_cursor < self.area.width() {
            self.wrap();
        }
        self.write(Slot::Glyph(self.glyph_at_cursor(grapheme, tails)));
        if grapheme.ends_with('\n') {
            self.new_line();
        }
    }

    fn glyph_at_cursor(&self, grapheme: &str, tails: usize) -> Glyph {
        Glyph::new(
            self.byte_cursor,
            self.byte_cursor + grapheme.len(),
            tails,
            Grapheme::from_bytes_iter(grapheme.bytes()),
        )
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

    /// Seals the row and starts a wrapped one, its text begins at `wrap_col`.
    fn wrap(&mut self) {
        self.seal();
        if self.wrap_col() > 0 {
            self.write(Slot::Marker('↩'));
            self.write(Slot::Blank);
        }
    }

    /// Returns the first col of a wrapped row free for text, after the markers.
    fn wrap_col(&self) -> usize {
        // The markers need room or they would wrap forever
        if self.area.width() > 2 { 2 } else { 0 }
    }

    pub(super) fn resize(&mut self, area: Area, scroll_offset: ScrollOffset) {
        self.area = area;
        // Counted back up by the rows we actually skip
        self.scroll_offset = ScrollOffset::new(scroll_offset.line, 0);
        self.skip_rows = scroll_offset.row;

        self.row_metadata.clear();
        self.row_metadata.resize_with(self.area.height(), || None);
        self.slots.clear();
        self.slots.resize_with(area.len(), || Slot::Blank);

        self.col_cursor = 0;
        self.row_cursor = 0;
        self.line_cursor = scroll_offset.line;
    }

    /// Lays `buffer` out from `scroll_offset`. The row is clamped to the rows the first line
    /// has, `scroll_offset()` returns where the layout really starts.
    pub(in crate::view) fn rebuild(
        &mut self,
        buffer: &Buffer,
        area: Area,
        scroll_offset: ScrollOffset,
    ) {
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
        }

        // A virtual glyph one past the last byte, so the cursor always has somewhere to be,
        // even in an empty buffer, and text can be appended at the end
        if reached_end {
            let end = buffer.rope().byte_len();
            self.write(Slot::Glyph(Glyph::new(end, end + 1, 0, Grapheme::blank())));
        }

        // The buffer ended before we skipped as many rows as asked, show its last row
        self.skip_rows = 0;
        if self.col_cursor > 0 {
            self.seal();
        }
    }
}
