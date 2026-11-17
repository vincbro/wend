use std::num::NonZeroUsize;

use serde::Deserialize;

/// How the view follows the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scroll {
    /// The rows kept between the cursor and the edge of the view while it scrolls.
    pub margin: usize,
}

/// How text is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    /// The columns between tab stops. Never zero, a tab would have no width.
    pub tab_width: NonZeroUsize,
}

/// The columns at the left of the view that hold the line numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineNumbers {
    /// The fewest digits the numbers make room for, so the text only moves when a file
    /// passes that many digits of lines.
    pub min_digits: usize,
    /// The blank columns between the numbers and the text.
    pub padding: u16,
}
