/// The overlays that take keys, so a place that reads actions as text can say whose
/// group it accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    Test,
    Command,
    Search,
    Find,
}

/// What the search looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKind {
    /// The files of the project, a result being a path.
    Files,
    /// The text in the files of the project, a result being `path:line:text`.
    Text,
}

impl SearchKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Text => "text",
        }
    }

    /// The kind `name` means, the inverse of `name`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "files" => Some(Self::Files),
            "text" => Some(Self::Text),
            _ => None,
        }
    }
}

/// What an overlay does, as a group for each overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayAction {
    Test(TestOverlayAction),
    Command(CommandAction),
    Search(SearchAction),
    Find(FindAction),
}

/// What the test overlay does. It has no actions yet, so there is no value of this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestOverlayAction {}

/// What the command line does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAction {
    /// Closes the command line and runs the action its text names.
    Run,
}

/// What the search overlay does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchAction {
    /// Closes the search and opens the file on the line the results' cursor is on.
    Pick,
}

/// What the prompt that searches a text does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindAction {
    /// Closes the prompt and keeps the match it selected, and the query for `n` and `p`.
    Confirm,
    /// Closes the prompt and puts the selection back to where it was.
    Close,
}

impl OverlayAction {
    /// The name a config gives this action.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Test(action) => match action {},
            Self::Command(CommandAction::Run) => "run_command",
            Self::Search(SearchAction::Pick) => "pick_result",
            Self::Find(FindAction::Confirm) => "confirm_find",
            Self::Find(FindAction::Close) => "close_find",
        }
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Test(action) => match action {},
            Self::Command(CommandAction::Run) => "run the command typed",
            Self::Search(SearchAction::Pick) => "open the file on the line picked",
            Self::Find(FindAction::Confirm) => "keep the match selected",
            Self::Find(FindAction::Close) => "close the search",
        }
    }

    /// The action of the overlay `kind` that `name` means, the inverse of `name`.
    #[must_use]
    pub fn from_name(kind: OverlayKind, name: &str) -> Option<Self> {
        match (kind, name) {
            (OverlayKind::Command, "run_command") => Some(Self::Command(CommandAction::Run)),
            (OverlayKind::Search, "pick_result") => Some(Self::Search(SearchAction::Pick)),
            (OverlayKind::Find, "confirm_find") => Some(Self::Find(FindAction::Confirm)),
            (OverlayKind::Find, "close_find") => Some(Self::Find(FindAction::Close)),
            (
                OverlayKind::Test | OverlayKind::Command | OverlayKind::Search | OverlayKind::Find,
                _,
            ) => None,
        }
    }
}
