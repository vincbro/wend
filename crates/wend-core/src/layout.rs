use std::ops;

use crate::buffer::Buffer;

const TAB_WIDTH: usize = 4;

#[derive(Debug, Clone, Copy)]
pub struct Glyph {
    pub from_byte: usize,
    pub to_byte: usize,
    pub width: usize,
}

impl Glyph {
    #[must_use]
    pub fn new(from_byte: usize, to_byte: usize, width: usize) -> Self {
        Self {
            from_byte,
            to_byte,
            width,
        }
    }

    #[must_use]
    pub fn as_range(&self) -> ops::Range<usize> {
        self.from_byte..self.to_byte
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Slot {
    Glyph(Glyph),
    Tail { slot_idx: usize },
    Blank,
}

#[derive(Debug, Clone, Copy)]
pub struct Row<'a> {
    pub line: usize,
    pub slots: &'a [Slot],
}

#[derive(Debug, Clone)]
pub struct Layout {
    slots: Vec<Slot>,
    row_meta: Vec<usize>,
    // How wide each line is
    width: usize,
    // How many lines we need to display
    lines: usize,
    // From which line we start from
    line_offset: usize,
}

impl Layout {
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: vec![],
            row_meta: vec![],
            width: 0,
            lines: 0,
            line_offset: 0,
        }
    }

    #[must_use]
    pub const fn fits(&self, width: usize, lines: usize, line_offest: usize) -> bool {
        self.width == width && self.lines >= lines && line_offest == 0
    }

    // Gets the glyph, none if it's Blank
    #[must_use]
    pub fn glyph(&self, slot: Slot) -> Option<Glyph> {
        match slot {
            Slot::Glyph(glyph) => Some(glyph),
            Slot::Tail { slot_idx } => self.glyph(self.slot(slot_idx)?),
            Slot::Blank => None,
        }
    }

    #[must_use]
    pub fn slot(&self, slot_idx: usize) -> Option<Slot> {
        self.slots.get(slot_idx).copied()
    }

    #[must_use]
    pub fn slot_index(&self, row: usize, cursor: usize) -> usize {
        cursor + row * self.width
    }

    pub fn lines(&self) -> impl ExactSizeIterator<Item = Row<'_>> {
        (0..self.lines).map(|idx| {
            let start = idx * self.width;
            let end = start + self.width;
            Row {
                line: self.row_meta[idx],
                slots: &self.slots[start..end],
            }
        })
    }

    pub fn model(&mut self, buffer: &Buffer, width: usize, lines: usize, line_offest: usize) {
        self.width = width;
        self.lines = lines;
        self.line_offset = line_offest;
        self.row_meta.resize_with(lines, || 0);
        self.slots.resize_with(width * lines, || Slot::Blank);

        // The current row we are on in the slots
        let mut row: usize = 0;

        for (line_idx, line) in buffer.rope.lines().skip(line_offest).enumerate() {
            // The real line we are on (for the gutter)
            let line_idx = line_idx.saturating_add(line_offest);
            let mut byte_offset = buffer.rope.byte_of_line(line_idx);
            // Where in the current row we are
            let mut col: usize = 0;

            for grapheme in line.graphemes() {
                let glyph_end = byte_offset.saturating_add(grapheme.len());
                let glyph = Slot::Glyph(Glyph::new(byte_offset, glyph_end, grapheme.len()));

                self.write(row, col, glyph);

                // Here we will add linking logic for multiline slots

                byte_offset = glyph_end;

                if grapheme == "\t" {
                    let jumps = if col.is_multiple_of(TAB_WIDTH) {
                        TAB_WIDTH
                    } else {
                        TAB_WIDTH - (col % TAB_WIDTH)
                    };

                    let col_end = col.saturating_add(jumps);

                    let index = self.slot_index(row, col);

                    for col in col.saturating_add(1)..col_end {
                        self.write(row, col, Slot::Tail { slot_idx: index });
                    }

                    col = col_end;
                } else {
                    col = col.saturating_add(1);
                }
            }

            // Fill from the col to the end with blanks
            for col in col..width {
                self.write(row, col, Slot::Blank);
            }

            row = row.saturating_add(1);
        }
    }

    fn write(&mut self, row: usize, cursor: usize, slot: Slot) {
        let idx = cursor + row * self.width;
        self.slots[idx] = slot;
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}
