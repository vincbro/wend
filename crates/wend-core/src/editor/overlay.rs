use crate::{
    config,
    frame::Rect,
    input::{
        action::{Action, CommandAction, OverlayAction},
        key::Key,
    },
    layout::{Canvas, Element, Layout},
};

mod command_line;
pub(super) use command_line::CommandLine;
mod keymap;
mod test_overlay;
pub(super) use test_overlay::TestOverlay;

/// A window floating over the editor, which gets the keys while it is open.
///
/// It is drawn after everything else, so what is on top and what gets the keys are the same
/// thing.
#[derive(Debug)]
pub(super) enum Overlay {
    Test(Box<TestOverlay>),
    Command(Box<CommandLine>),
}

impl Overlay {
    /// Feeds `key` to the overlay's keys and runs the actions of the overlay's own group. The
    /// editor's are added to `requests` for it to run, which this clears first. A key the
    /// overlay doesn't bind is dropped, so it never reaches the editor under it.
    ///
    /// Fails when the overlay ran something that doesn't exist, for the editor to say so.
    pub(super) fn key_press(&mut self, key: Key, requests: &mut Vec<Action>) -> config::Result<()> {
        match self {
            Self::Test(overlay) => {
                overlay.key_press(key, requests);
                Ok(())
            }
            Self::Command(overlay) => overlay.key_press(key, requests),
        }
    }
}

impl Overlay {
    /// Does an action of an overlay's own group. One of another overlay's group is dropped.
    pub(super) fn act(&mut self, action: OverlayAction) {
        match (self, action) {
            (Self::Test(_), OverlayAction::Test(action)) => match action {},
            // Running it needs the key that did, so the command line did it in `key_press`
            (
                Self::Command(_),
                OverlayAction::Command(CommandAction::Run) | OverlayAction::Test(_),
            )
            | (Self::Test(_), OverlayAction::Command(_)) => {}
        }
    }
}

impl Overlay {
    /// Checks the rules of what the overlay holds. Debug builds only.
    pub(super) fn check_invariants(&self) {
        match self {
            Self::Test(overlay) => overlay.check_invariants(),
            Self::Command(overlay) => overlay.check_invariants(),
        }
    }
}

impl Element for Overlay {
    fn layout(&self) -> Layout {
        match self {
            Self::Test(overlay) => overlay.layout(),
            Self::Command(overlay) => overlay.layout(),
        }
    }

    fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
        match self {
            Self::Test(overlay) => overlay.render(rect, canvas),
            Self::Command(overlay) => overlay.render(rect, canvas),
        }
    }
}
