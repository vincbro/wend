use std::path::Path;

/// What the scratch does about the document it shows, which a text box has no notion of.
///
/// Not `Copy`, because an action that takes arguments owns them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScratchAction {
    /// Writes the document to the path, or to its own file when there is none.
    Save(Option<Box<Path>>),
}

impl ScratchAction {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Save(_) => "save",
        }
    }

    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Save(_) => "save",
        }
    }

    /// The action with arguments that `name` means, none when there are no arguments.
    pub(super) fn with_arguments(name: &str, arguments: &str) -> Option<Self> {
        match name {
            "save" | "write" | "s" | "w" if !arguments.is_empty() => {
                Some(Self::Save(Some(Path::new(arguments).into())))
            }
            _ => None,
        }
    }

    /// The action `name` means when it takes no arguments, the inverse of `name`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "save" | "write" | "s" | "w" => Self::Save(None),
            _ => return None,
        })
    }
}
