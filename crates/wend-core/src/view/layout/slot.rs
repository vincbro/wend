use std::ops;

use crate::frame::Grapheme;

/// Describes a grapheme as laid out: the bytes it covers, and how many tail slots follow
/// its own slot.
///
/// A slot is copied for every cell of every rebuild, so the numbers are kept narrow:
/// `buffer` refuses text too long for a `u32` byte offset, and a glyph is never wider than
/// a `u8` of columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(in crate::view) struct Glyph {
    from_byte: u32,
    to_byte: u32,
    tails: u8,
    grapheme: Grapheme,
}

impl Glyph {
    #[must_use]
    pub(in crate::view) fn new(
        from_byte: usize,
        to_byte: usize,
        tails: usize,
        grapheme: Grapheme,
    ) -> Self {
        Self {
            from_byte: byte_offset(from_byte),
            to_byte: byte_offset(to_byte),
            tails: columns(tails),
            grapheme,
        }
    }

    #[must_use]
    pub(in crate::view) fn start(self) -> usize {
        self.from_byte as usize
    }

    #[must_use]
    pub(in crate::view) fn end(self) -> usize {
        self.to_byte as usize
    }

    #[must_use]
    pub(in crate::view) fn tails(self) -> usize {
        usize::from(self.tails)
    }

    #[must_use]
    pub(in crate::view) fn grapheme(self) -> Grapheme {
        self.grapheme
    }

    #[must_use]
    pub(in crate::view) fn to_range(self) -> ops::Range<usize> {
        self.start()..self.end()
    }
}

/// Narrows a byte offset to what a `Glyph` stores. Debug builds panic when it doesn't fit,
/// release builds keep the largest offset that does.
fn byte_offset(value: usize) -> u32 {
    let narrowed = u32::try_from(value);
    debug_assert!(
        narrowed.is_ok(),
        "buffers over u32::MAX bytes are refused, got an offset of {value}"
    );
    narrowed.unwrap_or(u32::MAX)
}

/// Narrows a count of columns to what a `Glyph` stores, the same way as `byte_offset`.
fn columns(value: usize) -> u8 {
    let narrowed = u8::try_from(value);
    debug_assert!(
        narrowed.is_ok(),
        "a grapheme is at most 31 bytes, so it is nowhere near u8::MAX columns, got {value}"
    );
    narrowed.unwrap_or(u8::MAX)
}

/// Locates a slot in a view's layout grid, relative to the view, not the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::view) struct SlotPosition {
    pub col: usize,
    pub row: usize,
}

impl SlotPosition {
    #[must_use]
    pub(in crate::view) fn new(col: usize, row: usize) -> Self {
        Self { col, row }
    }
}

/// Says what one cell of the layout grid holds.
#[derive(Debug, Clone, Copy)]
pub(in crate::view) enum Slot {
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
pub(in crate::view) struct ScrollOffset {
    pub line: usize,
    pub row: usize,
}

impl ScrollOffset {
    #[must_use]
    pub(in crate::view) fn new(line: usize, row: usize) -> Self {
        Self { line, row }
    }
}

/// Pairs a row of slots with its metadata.
#[derive(Debug, Clone, Copy)]
pub(in crate::view) struct Row<'a> {
    pub metadata: RowMetadata,
    pub slots: &'a [Slot],
}

/// Describes what a row shows. The byte range and `len` stop at its last used slot, so the
/// gap left by a wide glyph that wrapped isn't part of the row.
#[derive(Debug, Clone, Copy)]
pub(in crate::view) struct RowMetadata {
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
            "a layout writes one slot per cell on every rebuild, got {} bytes",
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
