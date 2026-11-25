use crate::input::{
    action::Action,
    key::{Key, KeyTree},
};

/// Turns keys into actions for one overlay, one key at a time, from the key tree it was
/// opened with.
///
/// `pending` is the combo so far, and always leads to a node of `tree`.
#[derive(Debug)]
pub(super) struct Keymap {
    tree: KeyTree,
    pending: Vec<Key>,
}

impl Keymap {
    pub(super) fn new(tree: KeyTree) -> Self {
        Self {
            tree,
            pending: vec![],
        }
    }

    /// Adds `key` to the combo and returns the actions to run: a leaf's. Empty while the
    /// combo waits for its next key, and after a key nothing binds, which drops the combo.
    #[must_use]
    pub(super) fn press(&mut self, key: Key) -> &[Action] {
        self.pending.push(key);
        let found = self.tree.get(&self.pending);
        if !matches!(found, Some(KeyTree::Node { .. })) {
            self.pending.clear();
        }
        self.check_invariants();
        match found {
            Some(KeyTree::Leaf(actions)) => actions,
            _ => &[],
        }
    }

    /// Checks the rules every call has to leave the keymap in. Debug builds only.
    pub(super) fn check_invariants(&self) {
        if !cfg!(debug_assertions) {
            return;
        }

        if self.pending.is_empty() {
            return;
        }
        assert!(
            matches!(self.tree.get(&self.pending), Some(KeyTree::Node { .. })),
            "press drops a combo that is not the start of a longer one, so `pending` leads to a node"
        );
    }
}

#[cfg(test)]
mod fuzz;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Config,
        input::{
            action::{EditorAction, OverlayKind},
            key::KeyCode,
        },
    };

    fn keymap() -> Keymap {
        Keymap::new(
            Config::default()
                .keys
                .overlay_tree(OverlayKind::Test)
                .unwrap(),
        )
    }

    #[test]
    fn escape_runs_close_overlay() {
        assert!(matches!(
            keymap().press(Key::new(KeyCode::Esc)),
            [Action::Editor(EditorAction::CloseOverlay)]
        ));
    }

    #[test]
    fn a_key_nothing_binds_does_nothing() {
        assert_eq!(keymap().press(Key::new(KeyCode::Char('z'))), []);
    }
}
