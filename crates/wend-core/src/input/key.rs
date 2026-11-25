use std::collections::HashMap;

use crate::input::action::Action;

/// One step of a combo a tree binds: a key, or any character that types itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPattern {
    Key(Key),
    /// Any key that has a `Key::character`, unless the node has a child for the key itself.
    Character,
}

/// A tree of key combos: each key leads to the actions it runs, or to a node of keys that
/// continue the combo. A key is never both, so a combo is done the moment it reaches a leaf.
#[derive(Debug, Clone)]
pub enum KeyTree {
    Leaf(Box<[Action]>),
    Node {
        children: HashMap<Key, Self>,
        /// Where a key goes that types a character and has no child of its own.
        character: Option<Box<Self>>,
    },
}

impl KeyTree {
    /// A node with no combos.
    #[must_use]
    pub fn new() -> Self {
        Self::Node {
            children: HashMap::new(),
            character: None,
        }
    }

    /// Binds `actions` to the combo `pattern`. `false`, with the tree maybe half grown, when the
    /// combo is empty or collides with another: one is the start of the other, or both are
    /// the same.
    pub fn insert(&mut self, pattern: &[KeyPattern], actions: Box<[Action]>) -> bool {
        let (
            Some((&first, rest)),
            Self::Node {
                children,
                character,
            },
        ) = (pattern.split_first(), self)
        else {
            return false;
        };
        match first {
            KeyPattern::Key(key) if rest.is_empty() => {
                children.insert(key, Self::Leaf(actions)).is_none()
            }
            KeyPattern::Key(key) => children
                .entry(key)
                .or_insert_with(Self::new)
                .insert(rest, actions),
            KeyPattern::Character if rest.is_empty() => {
                character.replace(Box::new(Self::Leaf(actions))).is_none()
            }
            KeyPattern::Character => character
                .get_or_insert_with(|| Box::new(Self::new()))
                .insert(rest, actions),
        }
    }

    /// Walks `keys` down from this tree, a key that types a character going to the node's
    /// character child when it has none of its own. `None` when a key isn't bound, or the walk
    /// goes on past a leaf.
    #[must_use]
    pub fn get(&self, keys: &[Key]) -> Option<&Self> {
        keys.iter().try_fold(self, |tree, key| match tree {
            Self::Node {
                children,
                character,
            } => children.get(key).or_else(|| {
                key.character()?;
                character.as_deref()
            }),
            Self::Leaf(_) => None,
        })
    }
}

impl Default for KeyTree {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    pub code: KeyCode,
    pub modifiers: Modifiers,
}

impl Key {
    #[must_use]
    pub fn new(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: Modifiers::default(),
        }
    }

    /// The character the key types, none for a named key or one held with control or alt.
    #[must_use]
    pub fn character(self) -> Option<char> {
        match self.code {
            KeyCode::Char(character) if !self.modifiers.control && !self.modifiers.alt => {
                Some(character)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KeyCode {
    Char(char),
    Enter,
    Esc,
    Backspace,
    Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Modifiers {
    pub alt: bool,
    pub control: bool,
    /// Never set on a `KeyCode::Char`, whose character already is the shifted one: `H`,
    /// not shift and `h`. The keymap binds characters that way, so a frontend has to drop
    /// shift from them.
    pub shift: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::action::{Action, EditorAction, ScratchAction};

    fn key(c: char) -> Key {
        Key::new(KeyCode::Char(c))
    }

    fn pattern(c: char) -> KeyPattern {
        KeyPattern::Key(key(c))
    }

    fn tree() -> KeyTree {
        let mut tree = KeyTree::new();
        assert!(tree.insert(
            &[pattern('q')],
            Box::new([Action::Editor(EditorAction::Quit)])
        ));
        assert!(tree.insert(
            &[pattern('g'), pattern('s')],
            Box::new([Action::Scratch(ScratchAction::Save(None))])
        ));
        tree
    }

    #[test]
    fn walk_reaches_a_leaf() {
        let tree = tree();
        assert!(matches!(
            tree.get(&[key('g'), key('s')]),
            Some(KeyTree::Leaf(actions)) if matches!(actions[..], [Action::Scratch(ScratchAction::Save(None))])
        ));
    }

    #[test]
    fn walk_stops_on_a_node() {
        let tree = tree();
        assert!(
            matches!(tree.get(&[key('g')]), Some(KeyTree::Node { .. })),
            "a prefix waits for the next key"
        );
    }

    #[test]
    fn unbound_key_is_none() {
        let tree = tree();
        assert!(tree.get(&[key('x')]).is_none(), "nothing binds x");
        assert!(
            tree.get(&[key('g'), key('x')]).is_none(),
            "g binds a node, but not x under it"
        );
    }

    #[test]
    fn walk_past_a_leaf_is_none() {
        let tree = tree();
        assert!(
            tree.get(&[key('q'), key('q')]).is_none(),
            "a leaf ends the combo"
        );
    }

    #[test]
    fn modifiers_make_a_different_key() {
        let tree = tree();
        let control_q = Key {
            code: KeyCode::Char('q'),
            modifiers: Modifiers {
                control: true,
                ..Modifiers::default()
            },
        };
        assert!(
            tree.get(&[control_q]).is_none(),
            "q is bound, control and q is not"
        );
    }

    #[test]
    fn insert_refuses_colliding_combos() {
        let mut tree = tree();
        assert!(
            !tree.insert(
                &[pattern('q'), pattern('x')],
                Box::new([Action::Scratch(ScratchAction::Save(None))])
            ),
            "q is a leaf, nothing continues it"
        );
        assert!(
            !tree.clone().insert(
                &[pattern('g')],
                Box::new([Action::Scratch(ScratchAction::Save(None))])
            ),
            "g starts a combo, so it can't be a leaf"
        );
        assert!(
            !tree.insert(&[], Box::new([Action::Scratch(ScratchAction::Save(None))])),
            "an empty combo is no combo"
        );
    }

    #[test]
    fn a_character_pattern_catches_the_keys_that_type() {
        let mut tree = tree();
        assert!(tree.insert(
            &[KeyPattern::Character],
            Box::new([Action::Scratch(ScratchAction::Save(None))])
        ));

        assert!(
            matches!(tree.get(&[key('z')]), Some(KeyTree::Leaf(_))),
            "z types"
        );
        assert!(
            matches!(
                tree.get(&[key('q')]),
                Some(KeyTree::Leaf(actions)) if matches!(actions[..], [Action::Editor(EditorAction::Quit)])
            ),
            "a key with a child of its own doesn't fall back"
        );
        assert!(
            tree.get(&[Key::new(KeyCode::Enter)]).is_none(),
            "enter types no character"
        );
        let control_z = Key {
            code: KeyCode::Char('z'),
            modifiers: Modifiers {
                control: true,
                ..Modifiers::default()
            },
        };
        assert!(tree.get(&[control_z]).is_none(), "control makes a command");
        assert!(
            tree.get(&[key('g'), key('z')]).is_none(),
            "g has no character child of its own"
        );
    }

    #[test]
    fn two_character_patterns_collide() {
        let mut tree = KeyTree::new();
        let save = || Box::new([Action::Scratch(ScratchAction::Save(None))]);
        assert!(tree.insert(&[KeyPattern::Character], save()));
        assert!(!tree.insert(&[KeyPattern::Character], save()));
    }
}
