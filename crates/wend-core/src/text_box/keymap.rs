use std::mem;

use crate::{
    config::{self, KeyTrees, Keys, Target},
    input::{
        Mode,
        action::Action,
        key::{Key, KeyTree},
    },
};

/// Turns keys into actions, one key at a time, with a key tree for each mode. A character
/// types because the tree binds `<char>`, so nothing here is hard-coded.
///
/// `pending` is the combo so far. It always leads to a node of `pending_mode`'s tree,
/// which is the mode the combo started in.
#[derive(Debug, Clone)]
pub(super) struct Keymap {
    /// Who the keys are for, kept to build them again when the config changes.
    target: Target,
    trees: KeyTrees,
    pending: Vec<Key>,
    pending_mode: Mode,
    /// Scratch space for `press_copied`, kept so that its memory is reused.
    copied: Vec<Action>,
}

impl Keymap {
    /// Fails when the keys of `keys` for `target` can't make key trees.
    pub(super) fn new(keys: &Keys, target: Target) -> config::Result<Self> {
        Ok(Self {
            target,
            trees: keys.trees(target)?,
            pending: vec![],
            pending_mode: Mode::Normal,
            copied: vec![],
        })
    }

    /// Follows `keys` from now on and drops the combo being typed. Keys that fail leave the
    /// keymap as it was.
    pub(super) fn set_keys(&mut self, keys: &Keys) -> config::Result<()> {
        self.trees = keys.trees(self.target)?;
        self.pending.clear();
        Ok(())
    }

    /// Like `press`, but the actions come out in a vector the keymap lends, so they can be
    /// run while the keymap is free. Give it back with `give_back` for its memory to be reused.
    #[must_use]
    pub(super) fn press_copied(&mut self, mode: Mode, key: Key) -> Vec<Action> {
        let mut copied = mem::take(&mut self.copied);
        copied.clear();
        copied.extend_from_slice(self.press(mode, key));
        copied
    }

    pub(super) fn give_back(&mut self, copied: Vec<Action>) {
        self.copied = copied;
    }

    /// Adds `key` to the combo and returns the actions to run: a leaf's. Empty while the
    /// combo waits for its next key, and after a key nothing binds, which drops the combo.
    ///
    /// A combo started in another mode is dropped first, so a mode change from an action
    /// can't leave it pointing into the wrong tree.
    #[must_use]
    pub(super) fn press(&mut self, mode: Mode, key: Key) -> &[Action] {
        if mode != self.pending_mode {
            self.pending.clear();
            self.pending_mode = mode;
        }
        self.pending.push(key);
        let found = self.trees.get(mode).get(&self.pending);
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
            matches!(
                self.trees.get(self.pending_mode).get(&self.pending),
                Some(KeyTree::Node { .. })
            ),
            "press drops a combo that is not the start of a longer one, so `pending` leads to a node"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{Config, Target},
        input::{
            action::TextBoxAction,
            key::{KeyCode, Modifiers},
        },
        text_box::{Direction, Motion},
    };

    fn key(character: char) -> Key {
        Key::new(KeyCode::Char(character))
    }

    fn default_keymap() -> Keymap {
        Keymap::new(&Config::default().keys, Target::TextBox).unwrap()
    }

    fn actions<'a>(keymap: &'a Keymap, mode: Mode, keys: &[Key]) -> &'a [Action] {
        match keymap.trees.get(mode).get(keys) {
            Some(KeyTree::Leaf(actions)) => actions,
            other => panic!("{keys:?} in {mode:?} is no leaf: {other:?}"),
        }
    }

    #[test]
    fn v_toggles_between_normal_and_select() {
        let keymap = default_keymap();
        assert!(matches!(
            actions(&keymap, Mode::Normal, &[key('v')]),
            [Action::TextBox(TextBoxAction::Mode(Mode::Select))]
        ));
        assert!(matches!(
            actions(&keymap, Mode::Select, &[key('v')]),
            [Action::TextBox(TextBoxAction::Mode(Mode::Normal))]
        ));
    }

    #[test]
    fn g_starts_a_combo() {
        let keymap = default_keymap();
        assert!(matches!(
            keymap.trees.get(Mode::Normal).get(&[key('g')]),
            Some(KeyTree::Node { .. })
        ));
        assert!(matches!(
            actions(&keymap, Mode::Normal, &[key('g'), key('e')]),
            [Action::TextBox(TextBoxAction::Motion(Motion::File(
                Direction::Forward
            )))]
        ));
    }

    #[test]
    fn o_opens_a_line_then_inserts() {
        let keymap = default_keymap();
        assert!(matches!(
            actions(&keymap, Mode::Normal, &[key('o')]),
            [
                Action::TextBox(TextBoxAction::OpenLine),
                Action::TextBox(TextBoxAction::Mode(Mode::Insert))
            ]
        ));
    }

    #[test]
    fn combo_waits_then_runs() {
        let mut keymap = default_keymap();
        assert!(keymap.press(Mode::Normal, key('g')).is_empty(), "g waits");
        assert!(matches!(
            keymap.press(Mode::Normal, key('s'))[..],
            [Action::TextBox(TextBoxAction::Motion(Motion::File(
                Direction::Backward
            )))]
        ));
        assert!(
            matches!(
                keymap.press(Mode::Normal, key('h'))[..],
                [Action::TextBox(TextBoxAction::Motion(Motion::Horizontal(
                    Direction::Backward
                )))]
            ),
            "the finished combo is gone, h starts fresh"
        );
    }

    #[test]
    fn unbound_key_drops_the_combo() {
        let mut keymap = default_keymap();
        assert!(keymap.press(Mode::Normal, key('g')).is_empty(), "g waits");
        assert!(
            keymap.press(Mode::Normal, key('z')).is_empty(),
            "normal mode doesn't type"
        );
        assert!(
            matches!(
                keymap.press(Mode::Normal, key('h'))[..],
                [Action::TextBox(TextBoxAction::Motion(Motion::Horizontal(
                    Direction::Backward
                )))]
            ),
            "h isn't read as g h"
        );
    }

    #[test]
    fn mode_change_drops_the_combo() {
        let mut keymap = default_keymap();
        assert!(keymap.press(Mode::Normal, key('g')).is_empty(), "g waits");
        assert!(
            matches!(
                keymap.press(Mode::Insert, key('s'))[..],
                [Action::TextBox(TextBoxAction::InsertTyped)]
            ),
            "s types instead of finishing g s"
        );
    }

    #[test]
    fn insert_types_unbound_characters() {
        let mut keymap = default_keymap();
        assert!(matches!(
            keymap.press(Mode::Insert, key('h'))[..],
            [Action::TextBox(TextBoxAction::InsertTyped)]
        ));
        let control_h = Key {
            code: KeyCode::Char('h'),
            modifiers: Modifiers {
                control: true,
                ..Modifiers::default()
            },
        };
        assert!(
            keymap.press(Mode::Insert, control_h).is_empty(),
            "a control key is a command, not text"
        );
    }

    #[test]
    fn normal_drops_unbound_characters() {
        let mut keymap = default_keymap();
        assert!(
            keymap.press(Mode::Normal, key('z')).is_empty(),
            "normal mode doesn't type"
        );
    }

    #[test]
    fn insert_binds_characters_to_typing() {
        let keymap = default_keymap();
        assert!(
            matches!(
                keymap.trees.get(Mode::Insert).get(&[key('h')]),
                Some(KeyTree::Leaf(actions)) if matches!(actions[..], [Action::TextBox(TextBoxAction::InsertTyped)])
            ),
            "the config types it"
        );
    }
}

#[cfg(test)]
mod fuzz;
