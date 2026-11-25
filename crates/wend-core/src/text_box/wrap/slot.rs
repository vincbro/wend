use std::ops;

use crate::{frame::Grapheme, number};

/// Describes a grapheme as laid out: the bytes it covers, and how many tail slots follow
/// its own slot.
///
/// A slot is copied for every cell of every rebuild, so the numbers are kept narrow:
/// `buffer` refuses text too long for a `u32` byte offset, and a glyph is never wider than
/// a `u8` of columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(in crate::text_box) struct Glyph {
    from_byte: u32,
    to_byte: u32,
    tails: u8,
    grapheme: Grapheme,
}

impl Glyph {
    #[must_use]
    pub(in crate::text_box) fn new(
        from_byte: usize,
        to_byte: usize,
        tails: usize,
        grapheme: Grapheme,
    ) -> Self {
        // buffer::MAX_LEN keeps offsets within a u32, and a grapheme is at most 31 bytes wide.
        Self {
            from_byte: number::exact(from_byte),
            to_byte: number::exact(to_byte),
            tails: number::exact(tails),
            grapheme,
        }
    }

    #[must_use]
    pub(in crate::text_box) fn start(self) -> usize {
        number::exact(self.from_byte)
    }

    #[must_use]
    pub(in crate::text_box) fn end(self) -> usize {
        number::exact(self.to_byte)
    }

    #[must_use]
    pub(in crate::text_box) fn tails(self) -> usize {
        usize::from(self.tails)
    }

    #[must_use]
    pub(in crate::text_box) fn grapheme(self) -> Grapheme {
        self.grapheme
    }

    #[must_use]
    pub(in crate::text_box) fn to_range(self) -> ops::Range<usize> {
        self.start()..self.end()
    }
}

/// Locates a slot in a view's wrap, relative to the view, not the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::text_box) struct SlotPosition {
    pub col: usize,
    pub row: usize,
}

impl SlotPosition {
    #[must_use]
    pub(in crate::text_box) fn new(col: usize, row: usize) -> Self {
        Self { col, row }
    }
}

/// Says what one cell of the wrap holds.
#[derive(Debug, Clone, Copy)]
pub(in crate::text_box) enum Slot {
    Glyph(Glyph),
    /// Fills the rest of a glyph wider than one slot. Holds the index of the glyph's own slot.
    Tail(usize),
    /// Shows a marker that isn't text, such as the ↩ that starts a wrapped row.
    Marker(char),
    Blank,
}

/// Marks the top of a view: the first line on screen and how many of its wrapped rows are
/// scrolled past, so the view can start partway through a line that wraps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::text_box) struct ScrollOffset {
    pub line: usize,
    pub row: usize,
}

impl ScrollOffset {
    #[must_use]
    pub(in crate::text_box) fn new(line: usize, row: usize) -> Self {
        Self { line, row }
    }
}

/// Pairs a row of slots with its metadata.
#[derive(Debug, Clone, Copy)]
pub(in crate::text_box) struct Row<'a> {
    pub metadata: RowMetadata,
    pub slots: &'a [Slot],
}

/// Describes what a row shows. The byte range and `len` stop at its last used slot, so the
/// gap left by a wide glyph that wrapped isn't part of the row.
#[derive(Debug, Clone, Copy)]
pub(in crate::text_box) struct RowMetadata {
    pub line: usize,
    pub from_byte: usize,
    pub to_byte: usize,
    /// Counts the slots the row uses, the rest of it is blank.
    pub len: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slot_stays_small() {
        assert!(
            size_of::<Slot>() <= 48,
            "a wrap writes one slot per cell on every rebuild, got {} bytes",
            size_of::<Slot>()
        );
    }

    #[test]
    fn a_glyph_gives_back_what_it_was_made_with() {
        let glyph = Glyph::new(10, 14, 1, Grapheme::from_char('漢'));
        assert_eq!(glyph.to_range(), 10..14, "the bytes it covers");
        assert_eq!(glyph.tails(), 1, "the tail slots after it");
    }
}
