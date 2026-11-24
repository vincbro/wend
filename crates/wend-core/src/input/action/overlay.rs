/// The overlays that take keys, so a place that reads actions as text can say whose
/// group it accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    Test,
}

/// What an overlay does, as a group for each overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayAction {
    Test(TestOverlayAction),
}

/// What the test overlay does. It has no actions yet, so there is no value of this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestOverlayAction {}

impl OverlayAction {
    /// The name a config gives this action.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Test(action) => match action {},
        }
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Test(action) => match action {},
        }
    }

    /// The action of the overlay `kind` that `name` means, the inverse of `name`.
    #[must_use]
    pub fn from_name(kind: OverlayKind, _name: &str) -> Option<Self> {
        match kind {
            OverlayKind::Test => None,
        }
    }
}
