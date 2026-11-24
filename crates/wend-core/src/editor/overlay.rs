use crate::{
    frame::Rect,
    input::{
        action::{EditorAction, OverlayAction, ViewAction},
        key::Key,
    },
    layout::{Canvas, Element, Layout},
};

mod keymap;
mod test_overlay;
pub(super) use test_overlay::TestOverlay;

/// A window floating over the editor, which gets the keys while it is open.
///
/// It is drawn after everything else, so what is on top and what gets the keys are the same
/// thing.
#[derive(Debug)]
pub(super) enum Overlay {
    Test(TestOverlay),
}

impl Overlay {
    /// Feeds `key` to the overlay's keys and runs the actions of the overlay's own group. The
    /// editor's are added to `requests` for it to run, which this clears first. A key the
    /// overlay doesn't bind is dropped, so it never reaches the editor under it.
    pub(super) fn key_press(&mut self, key: Key, requests: &mut Vec<EditorAction>) {
        match self {
            Self::Test(overlay) => overlay.key_press(key, requests),
        }
    }

    /// Does a view action the overlay is the target of. No overlay has actions of its own yet,
    /// so it is dropped.
    pub(super) fn perform(&mut self, _action: ViewAction) {
        match self {
            Self::Test(_) => {}
        }
    }
}

impl Overlay {
    /// Does an action of an overlay's own group. One of another overlay's group is dropped.
    pub(super) fn act(&mut self, action: OverlayAction) {
        match (self, action) {
            (Self::Test(_), OverlayAction::Test(action)) => match action {},
        }
    }
}

impl Overlay {
    /// Checks the rules of what the overlay holds. Debug builds only.
    pub(super) fn check_invariants(&self) {
        match self {
            Self::Test(overlay) => overlay.check_invariants(),
        }
    }
}

impl Element for Overlay {
    fn layout(&self) -> Layout {
        match self {
            Self::Test(overlay) => overlay.layout(),
        }
    }

    fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
        match self {
            Self::Test(overlay) => overlay.render(rect, canvas),
        }
    }
}
