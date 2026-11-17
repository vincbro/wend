/// How far above the bottom of the frame a cell was drawn. A cell only replaces a cell on the
/// same layer or a lower one, so what floats above something stays above it whatever is drawn
/// first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Layer(u8);

impl Layer {
    /// The layer everything is drawn on until an element floats above it.
    pub const BOTTOM: Self = Self(0);

    /// Returns the layer `steps` above this one.
    #[must_use]
    pub(crate) fn lift(self, steps: u8) -> Self {
        Self(self.0 + steps)
    }
}
