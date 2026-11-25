/// The overlays that take keys, so a place that reads actions as text can say whose
/// group it accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    Test,
    Command,
}

/// What an overlay does, as a group for each overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayAction {
    Test(TestOverlayAction),
    Command(CommandAction),
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

impl OverlayAction {
    /// The name a config gives this action.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Test(action) => match action {},
            Self::Command(CommandAction::Run) => "run_command",
        }
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Test(action) => match action {},
            Self::Command(CommandAction::Run) => "run the command typed",
        }
    }

    /// The action of the overlay `kind` that `name` means, the inverse of `name`.
    #[must_use]
    pub fn from_name(kind: OverlayKind, name: &str) -> Option<Self> {
        match (kind, name) {
            (OverlayKind::Command, "run_command") => Some(Self::Command(CommandAction::Run)),
            (OverlayKind::Test | OverlayKind::Command, _) => None,
        }
    }
}
