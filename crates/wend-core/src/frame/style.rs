use crate::syntax::Highlight;

/// What a cell looks like, as layers drawn from the bottom up: the content, the selection
/// over it, and the cursor over that.
///
/// The core only says which layers apply. The frontend's theme decides what each one
/// changes, so a selected keyword can keep its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub content: Content,
    pub selected: bool,
    /// Set on every cell of the grapheme under the head, while `Frame::cursor` is the one
    /// position where that grapheme starts.
    pub cursor: bool,
}

/// The bottom layer: what the text in a cell is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Content {
    #[default]
    Plain,
    Text(Highlight),
    LineNumber,
    CurrentLineNumber,
    Marker,
    /// The lines around a floating pane.
    Border,
    /// A message that says something went wrong.
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_style_stays_three_bytes() {
        assert_eq!(
            size_of::<Style>(),
            3,
            "every cell of every frame carries one"
        );
    }
}
