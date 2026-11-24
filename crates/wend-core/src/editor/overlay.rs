use crate::{
    frame::Rect,
    input::{
        action::{OverlayAction, ViewAction},
        key::{Key, KeyCode},
    },
    layout::{Canvas, Element, Layout},
};

mod test_overlay;
pub(super) use test_overlay::TestOverlay;

/// What an overlay did with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyOutcome {
    /// The overlay used the key, so the editor under it doesn't see it.
    Consumed,
    /// The overlay is done and goes away.
    Closed,
}

/// A window floating over the editor, which gets the keys while it is open.
///
/// It is drawn after everything else, so what is on top and what gets the keys are the same
/// thing.
#[derive(Debug)]
pub(super) enum Overlay {
    Test(TestOverlay),
}

impl Overlay {
    /// Handles `key` and says whether the overlay is done with it. A key the overlay doesn't
    /// use doesn't reach the editor under it either.
    ///
    /// Escape closes every overlay, so it is decided here and no overlay can take it.
    pub(super) fn key(&mut self, key: Key) -> KeyOutcome {
        if key.code == KeyCode::Esc {
            return KeyOutcome::Closed;
        }
        match self {
            Self::Test(_) => KeyOutcome::Consumed,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn overlay() -> Overlay {
        Overlay::Test(TestOverlay::new(&Config::default()).unwrap())
    }

    #[test]
    fn escape_closes_the_overlay() {
        assert_eq!(
            overlay().key(Key::new(KeyCode::Esc)),
            KeyOutcome::Closed,
            "the way out of every overlay"
        );
    }

    #[test]
    fn escape_closes_the_overlay_with_a_modifier_held() {
        let mut key = Key::new(KeyCode::Esc);
        key.modifiers.alt = true;

        assert_eq!(overlay().key(key), KeyOutcome::Closed, "alt-escape too");
    }

    #[test]
    fn every_other_key_is_taken_by_the_overlay() {
        let mut overlay = overlay();

        for code in [KeyCode::Char('i'), KeyCode::Enter, KeyCode::Tab] {
            assert_eq!(
                overlay.key(Key::new(code)),
                KeyOutcome::Consumed,
                "{code:?} doesn't reach the editor"
            );
        }
    }
}
