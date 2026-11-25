use std::path::Path;

use super::SearchKind;
use crate::input;

/// What any target can do, wherever the keys came from. It needs nothing of a document or of
/// text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorAction {
    /// Asks the frontend for the file at the path, which the editor then shows.
    Open(Box<Path>),
    /// Asks the frontend for the file at the path, which the editor then shows with the cursor
    /// on the line, counted from 1.
    OpenAt {
        path: Box<Path>,
        line: usize,
    },
    /// Opens the overlay that tries out arranging two windows.
    OpenTestOverlay,
    /// Opens the command line, which runs the action its text names.
    OpenCommandLine,
    /// Opens the search, which lists what the frontend finds and opens the one picked.
    OpenSearch(SearchKind),
    /// Closes the overlay that has the keys.
    CloseOverlay,
    Quit,
}

impl EditorAction {
    /// The action with arguments that `name` means, none when `name` takes none.
    pub(super) fn with_arguments(name: &str, arguments: &str) -> Option<input::Result<Self>> {
        match name {
            "open" | "o" if arguments.is_empty() => {
                Some(Err(input::Error::MissingArguments(name.into())))
            }
            "open" | "o" => Some(Ok(Self::Open(Path::new(arguments).into()))),
            "open_at" => Some(Self::open_at(arguments)),
            "open_search" if !arguments.is_empty() => Some(
                SearchKind::from_name(arguments)
                    .map(Self::OpenSearch)
                    .ok_or(input::Error::InvalidArguments {
                        name: "open_search",
                        expected: "`files` or `text`",
                    }),
            ),
            _ => None,
        }
    }

    /// Reads the arguments of `open_at`: a path, a colon and a line, counted from 1.
    fn open_at(arguments: &str) -> input::Result<Self> {
        let invalid = || input::Error::InvalidArguments {
            name: "open_at",
            expected: "a path and a line, as `path:line`",
        };
        let (path, line) = arguments.rsplit_once(':').ok_or_else(invalid)?;
        let line = line.parse().map_err(|_| invalid())?;
        Ok(Self::OpenAt {
            path: Path::new(path).into(),
            line,
        })
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Open(_) => "open",
            Self::OpenAt { .. } => "open_at",
            Self::OpenTestOverlay => "open_test_overlay",
            Self::OpenCommandLine => "open_command_line",
            Self::OpenSearch(_) => "open_search",
            Self::CloseOverlay => "close_overlay",
            Self::Quit => "quit",
        }
    }

    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Open(_) => "open a file",
            Self::OpenAt { .. } => "open a file at a line",
            Self::OpenTestOverlay => "two windows side by side",
            Self::OpenCommandLine => "type a command",
            Self::OpenSearch(SearchKind::Files) => "search for a file",
            Self::OpenSearch(SearchKind::Text) => "search for text in the files",
            Self::CloseOverlay => "close the overlay",
            Self::Quit => "quit",
        }
    }

    /// The action `name` means when it takes no arguments, the inverse of `name`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "open_test_overlay" => Self::OpenTestOverlay,
            "open_command_line" => Self::OpenCommandLine,
            "open_search" => Self::OpenSearch(SearchKind::Files),
            "close_overlay" => Self::CloseOverlay,
            "quit" | "q" => Self::Quit,
            _ => return None,
        })
    }
}
