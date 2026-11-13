use crate::{
    config::KeyTrees,
    input::{
        Mode,
        action::Action,
        key::{Key, KeyCode, KeyTree},
    },
};

/// Turns keys into actions, one key at a time, with a key tree for each mode.
///
/// `pending` is the combo so far. It always leads to a node of `pending_mode`'s tree,
/// which is the mode the combo started in.
#[derive(Debug, Clone)]
pub(super) struct Keymap {
    trees: KeyTrees,
    pending: Vec<Key>,
    pending_mode: Mode,
    /// Holds the action of a character typing itself, so `press` can lend it like a leaf's.
    typed: [Action; 1],
}

impl Keymap {
    #[must_use]
    pub(super) fn new(trees: KeyTrees) -> Self {
        Self {
            trees,
            pending: vec![],
            pending_mode: Mode::Normal,
            // Overwritten before it is lent out
            typed: [Action::Insert(' ')],
        }
    }

    /// Adds `key` to the combo and returns the actions to run: a leaf's, or a character
    /// typing itself in insert mode. Empty while the combo waits for its next key, and
    /// after a key nothing binds, which drops the combo.
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
        match self.trees.get(mode).get(&self.pending) {
            Some(KeyTree::Node { .. }) => &[],
            Some(KeyTree::Leaf(actions)) => {
                self.pending.clear();
                actions
            }
            None => {
                let is_combo = self.pending.len() > 1;
                self.pending.clear();
                typed_action(&mut self.typed, mode, key, is_combo)
            }
        }
    }
}

/// A character the insert tree doesn't bind types itself, unless it ends a combo or comes
/// with control or alt. Anything else does nothing. Takes the scratch space and not the
/// keymap, so `press` can keep the trees borrowed.
fn typed_action(typed: &mut [Action; 1], mode: Mode, key: Key, is_combo: bool) -> &[Action] {
    let is_typed = !key.modifiers.control && !key.modifiers.alt;
    match (mode, key.code) {
        (Mode::Insert, KeyCode::Char(c)) if is_typed && !is_combo => {
            *typed = [Action::Insert(c)];
            typed
        }
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Keys,
        input::key::Modifiers,
        view::{Direction, Motion},
    };

    fn key(character: char) -> Key {
        Key::new(KeyCode::Char(character))
    }

    fn default_keymap() -> Keymap {
        Keymap::new(Keys::default().trees().unwrap())
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
            [Action::Mode(Mode::Select)]
        ));
        assert!(matches!(
            actions(&keymap, Mode::Select, &[key('v')]),
            [Action::Mode(Mode::Normal)]
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
            [Action::Motion(Motion::File(Direction::Forward))]
        ));
    }

    #[test]
    fn o_opens_a_line_then_inserts() {
        let keymap = default_keymap();
        assert!(matches!(
            actions(&keymap, Mode::Normal, &[key('o')]),
            [Action::OpenLine, Action::Mode(Mode::Insert)]
        ));
    }

    #[test]
    fn combo_waits_then_runs() {
        let mut keymap = default_keymap();
        assert!(keymap.press(Mode::Normal, key('g')).is_empty(), "g waits");
        assert!(matches!(
            keymap.press(Mode::Normal, key('s'))[..],
            [Action::Motion(Motion::File(Direction::Backward))]
        ));
        assert!(
            matches!(keymap.press(Mode::Normal, key('q'))[..], [Action::Quit]),
            "the finished combo is gone, q starts fresh"
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
            matches!(keymap.press(Mode::Normal, key('q'))[..], [Action::Quit]),
            "q isn't read as g q"
        );
    }

    #[test]
    fn mode_change_drops_the_combo() {
        let mut keymap = default_keymap();
        assert!(keymap.press(Mode::Normal, key('g')).is_empty(), "g waits");
        assert!(
            matches!(
                keymap.press(Mode::Insert, key('s'))[..],
                [Action::Insert('s')]
            ),
            "s types instead of finishing g s"
        );
    }

    #[test]
    fn insert_types_unbound_characters() {
        let mut keymap = default_keymap();
        assert!(matches!(
            keymap.press(Mode::Insert, key('h'))[..],
            [Action::Insert('h')]
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
    fn insert_leaves_characters_unbound() {
        let keymap = default_keymap();
        assert!(
            keymap.trees.get(Mode::Insert).get(&[key('h')]).is_none(),
            "the editor types it"
        );
    }
}
