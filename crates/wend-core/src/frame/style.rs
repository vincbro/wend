use crate::syntax::Highlight;

/// What a cell looks like, as layers drawn from the bottom up: the content, the line the cursor
/// is on over it, the selection over that, and the cursor over all of it.
///
/// The core only says which layers apply. The frontend's theme decides what each one
/// changes, so a selected keyword can keep its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub content: Content,
    /// Set on every cell of the line the head is on, the blank cells to the end of each of its
    /// rows included.
    pub line: bool,
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
    fn a_style_stays_four_bytes() {
        assert_eq!(
            size_of::<Style>(),
            4,
            "every cell of every frame carries one"
        );
    }
}
