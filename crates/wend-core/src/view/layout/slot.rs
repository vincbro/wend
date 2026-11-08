use std::ops;

use crate::frame::Grapheme;

/// Describes a grapheme as laid out: the bytes it covers, and how many tail slots follow
/// its own slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(in crate::view) struct Glyph {
    pub from_byte: usize,
    pub to_byte: usize,
    pub tails: usize,
    pub grapheme: Grapheme,
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
            from_byte,
            to_byte,
            tails,
            grapheme,
        }
    }

    #[must_use]
    pub(in crate::view) fn to_range(self) -> ops::Range<usize> {
        self.from_byte..self.to_byte
    }
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
