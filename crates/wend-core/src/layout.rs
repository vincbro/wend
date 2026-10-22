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
    Misc(char),
    Blank,
}

impl Slot {
    #[must_use]
    pub const fn is_glyph(self) -> bool {
        matches!(self, Self::Glyph(_))
    }

    #[must_use]
    pub const fn is_tail(self) -> bool {
        matches!(self, Self::Tail { .. })
    }

    #[must_use]
    pub const fn is_misc(self) -> bool {
        matches!(self, Self::Misc(_))
    }

    #[must_use]
    pub const fn is_blank(self) -> bool {
        matches!(self, Self::Blank)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Row<'a> {
    pub meta: Option<RowMeta>,
    pub slots: &'a [Slot],
}

#[derive(Debug, Clone, Copy)]
pub struct RowMeta {
    pub line: usize,
    pub from_byte: usize,
    pub to_byte: usize,
    // How many slots unitl the end of the line
    pub length: usize,
}

#[derive(Debug, Clone)]
pub struct Layout {
    slots: Vec<Slot>,
    row_meta: Vec<Option<RowMeta>>,
    // How wide each line is
    width: usize,
    // How many lines we need to display
    lines: usize,
    // From which line we start from
    line_offset: usize,

    row_ptr: usize,
    col_ptr: usize,
    line_ptr: usize,
    from_byte_ptr: usize,
    to_byte_ptr: usize,
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

            row_ptr: 0,
            col_ptr: 0,
            line_ptr: 0,
            from_byte_ptr: usize::MAX,
            to_byte_ptr: 0,
        }
    }

    #[must_use]
    pub const fn fits(&self, width: usize, lines: usize, line_offest: usize) -> bool {
        self.width == width && self.lines >= lines && self.line_offset == line_offest
    }

    // Gets the glyph, none if it's Blank
    #[must_use]
    pub fn glyph(&self, slot: Slot) -> Option<Glyph> {
        match slot {
            Slot::Glyph(glyph) => Some(glyph),
            Slot::Tail { slot_idx } => self.glyph(self.slot(slot_idx)?),
            Slot::Misc(_) | Slot::Blank => None,
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

    #[must_use]
    pub fn row(&self, index: usize) -> Option<Row<'_>> {
        let start = index * self.width;
        let end = start + self.width;
        Some(Row {
            meta: self.row_meta.get(index).copied()?,
            slots: self.slots.get(start..end)?,
        })
    }

    pub fn rows(&self) -> impl ExactSizeIterator<Item = Row<'_>> {
        (0..self.lines).map(|idx| {
            let start = idx * self.width;
            let end = start + self.width;
            Row {
                meta: self.row_meta[idx],
                slots: &self.slots[start..end],
            }
        })
    }

    fn new_line(&mut self) {
        self.seal();
        self.line_ptr = self.line_ptr.saturating_add(1);
    }

    /// Seals the current row, appends row meta data.
    fn seal(&mut self) {
        self.row_meta[self.row_ptr] = Some(RowMeta {
            line: self.line_ptr,
            from_byte: self.from_byte_ptr,
            to_byte: self.to_byte_ptr,
            length: self.col_ptr,
        });

        for _ in self.col_ptr..self.width {
            let idx = self.col_ptr + self.row_ptr * self.width;
            let Some(slot_mut) = self.slots.get_mut(idx) else {
                panic!("Should not be able to overflow");
            };
            *slot_mut = Slot::Blank;
            self.col_ptr = self.col_ptr.saturating_add(1);
        }

        self.from_byte_ptr = usize::MAX;
        self.to_byte_ptr = 0;

        self.col_ptr = 0;
        self.row_ptr = self.row_ptr.saturating_add(1);
    }

    /// Writes a single slot, handles auto sealing and overflow protection.
    fn write(&mut self, slot: Slot) {
        assert!(
            self.row_ptr < self.lines,
            "Trying to write outside the buffer"
        );
        assert!(self.col_ptr < self.width, "Trying to write outside the row");

        let idx = self.col_ptr + self.row_ptr * self.width;
        let Some(slot_mut) = self.slots.get_mut(idx) else {
            panic!("Should not be able to overflow");
        };
        *slot_mut = slot;

        if let Slot::Glyph(glyph) = slot {
            self.from_byte_ptr = self.from_byte_ptr.min(glyph.from_byte);
            self.to_byte_ptr = self.to_byte_ptr.max(glyph.to_byte);
        }

        self.col_ptr = self.col_ptr.saturating_add(1);

        if self.col_ptr >= self.width {
            self.seal();
            self.write(Slot::Misc('↩'));
        }
    }

    pub fn model(&mut self, buffer: &Buffer, width: usize, lines: usize, line_offest: usize) {
        self.width = width;
        self.lines = lines;
        self.line_offset = line_offest;
        self.row_meta.resize_with(lines, || None);
        self.slots.resize_with(width * lines, || Slot::Blank);

        self.col_ptr = 0;
        self.row_ptr = 0;
        self.line_ptr = line_offest;

        // The current row we are on in the slots

        let start = buffer.rope.byte_of_line(self.line_offset);
        let mut byte_ptr = start;

        for grapheme in buffer.rope.byte_slice(start..).graphemes() {
            if self.row_ptr >= self.lines {
                break;
            }

            let glyph_start = byte_ptr;
            byte_ptr = byte_ptr.saturating_add(grapheme.len());
            let glyph = Slot::Glyph(Glyph::new(glyph_start, byte_ptr, grapheme.len()));
            let col = self.col_ptr;
            let index = self.slot_index(self.row_ptr, col);
            self.write(glyph);
            // Here we will add linking logic for multiline slots
            if grapheme == "\t" {
                let jumps = if col.is_multiple_of(TAB_WIDTH) {
                    TAB_WIDTH
                } else {
                    TAB_WIDTH - (col % TAB_WIDTH)
                };
                for _ in 1..jumps {
                    self.write(Slot::Tail { slot_idx: index });
                }
            } else if grapheme.ends_with('\n') {
                self.new_line();
            }
        }

        if self.col_ptr > 0 {
            self.seal();
        }

        // for line in buffer.rope.lines().skip(self.line_offset) {
        //     if self.row_ptr >= self.lines {
        //         break;
        //     }
        //     // The real line we are on (for the gutter)
        //     let start_byte = buffer.rope.byte_of_line(self.line_ptr);
        //     let mut byte_offset = start_byte;
        //     // Where in the current row we are

        //     for grapheme in line.graphemes() {
        //         let glyph_start = byte_offset;
        //         byte_offset = byte_offset.saturating_add(grapheme.len());
        //         let glyph = Slot::Glyph(Glyph::new(glyph_start, byte_offset, grapheme.len()));
        //         let col = self.col_ptr;
        //         let index = self.slot_index(self.row_ptr, col);
        //         self.write(glyph);
        //         // Here we will add linking logic for multiline slots
        //         if grapheme == "\t" {
        //             let jumps = if col.is_multiple_of(TAB_WIDTH) {
        //                 TAB_WIDTH
        //             } else {
        //                 TAB_WIDTH - (col % TAB_WIDTH)
        //             };
        //             for _ in 1..jumps {
        //                 self.write(Slot::Tail { slot_idx: index });
        //             }
        //         }
        //     }
        //     self.new_line();
        // }
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}
