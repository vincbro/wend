use crate::buffer::Change;

/// A selection between two graphemes, both ends included. A cursor is a selection whose
/// anchor and head are the same grapheme.
///
/// Both ends are byte offsets where a grapheme starts. `View::set_selection` is what keeps
/// them on grapheme starts, a `Selection` on its own doesn't know about the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    anchor: usize,
    head: usize,
}

impl Selection {
    #[must_use]
    pub fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    /// A selection of the single grapheme at `byte`.
    #[must_use]
    pub fn point(byte: usize) -> Self {
        Self::new(byte, byte)
    }

    /// The end that stays put while extending.
    #[must_use]
    pub fn anchor(self) -> usize {
        self.anchor
    }

    /// The end that moves, where the cursor is drawn.
    #[must_use]
    pub fn head(self) -> usize {
        self.head
    }

    #[must_use]
    pub fn collapse(self) -> Self {
        Self::point(self.head)
    }

    /// Where the first selected grapheme starts.
    #[must_use]
    pub fn start(self) -> usize {
        self.anchor.min(self.head)
    }

    /// Where the last selected grapheme starts.
    #[must_use]
    pub fn end(self) -> usize {
        self.anchor.max(self.head)
    }

    /// The selection after an edit.
    #[must_use]
    pub fn map(self, change: Change) -> Self {
        Self::new(change.map_pos(self.anchor), change.map_pos(self.head))
    }
}
