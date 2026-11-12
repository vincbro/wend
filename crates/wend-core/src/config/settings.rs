use std::num::NonZeroUsize;

use serde::{Deserialize, Serialize};

/// How the view follows the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Scroll {
    /// The rows kept between the cursor and the edge of the view while it scrolls.
    pub margin: usize,
}

impl Default for Scroll {
    fn default() -> Self {
        Self { margin: 10 }
    }
}

/// How text is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Text {
    /// The columns between tab stops. Never zero, a tab would have no width.
    pub tab_width: NonZeroUsize,
}

impl Default for Text {
    fn default() -> Self {
        Self {
            tab_width: NonZeroUsize::new(4).expect("4 is not zero"),
        }
    }
}

/// The columns at the left of the view that hold the line numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LineNumbers {
    /// The fewest digits the numbers make room for, so the text only moves when a file
    /// passes that many digits of lines.
    pub min_digits: usize,
    /// The blank columns between the numbers and the text.
    pub padding: u16,
}

impl Default for LineNumbers {
    fn default() -> Self {
        Self {
            min_digits: 3,
            padding: 1,
        }
    }
}
