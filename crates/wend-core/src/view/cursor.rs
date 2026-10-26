use std::ops;

use crate::view::layout::Glyph;

#[derive(Debug, Clone, Copy)]
pub struct Cursor {
    pub from_byte: usize,
    pub to_byte: usize,
}

impl Cursor {
    pub fn new(from_byte: usize, to_byte: usize) -> Self {
        Self { from_byte, to_byte }
    }

    #[must_use]
    pub fn overlaps(&self, range: ops::Range<usize>) -> bool {
        self.from_byte < range.end && range.start < self.to_byte
    }

    pub fn set_glyph(&mut self, glyph: Glyph) {
        self.from_byte = glyph.from_byte;
        self.to_byte = glyph.to_byte;
    }

    pub fn is_glyph(&self, glyph: Glyph) -> bool {
        self.from_byte == glyph.from_byte && self.to_byte == glyph.to_byte
    }
}
