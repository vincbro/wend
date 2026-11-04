use std::collections::HashMap;

use crate::input::action::Action;

/// A keymap: each key leads to the actions it runs, or to a node of keys that continue the
/// combo. A key is never both, so a combo is done the moment it reaches a leaf.
#[derive(Debug, Clone)]
pub enum KeyTree {
    Leaf(Vec<Action>),
    Node {
        description: String,
        children: HashMap<Key, Self>,
    },
}

impl KeyTree {
    #[must_use]
    pub fn node(description: &str, children: Vec<(Key, Self)>) -> Self {
        Self::Node {
            description: description.to_owned(),
            children: children.into_iter().collect(),
        }
    }

    /// Walks `keys` down from this tree. `None` when a key isn't bound, or the walk goes
    /// on past a leaf.
    #[must_use]
    pub fn get(&self, keys: &[Key]) -> Option<&Self> {
        keys.iter().try_fold(self, |tree, key| match tree {
            Self::Node { children, .. } => children.get(key),
            Self::Leaf(_) => None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    pub code: KeyCode,
    pub modifiers: Modifiers,
}

impl Key {
    #[must_use]
    pub fn plain(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: Modifiers::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Char(char),
    Enter,
    Esc,
    Backspace,
    Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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
    use crate::input::action::Action;

    fn key(c: char) -> Key {
        Key::plain(KeyCode::Char(c))
    }

    fn tree() -> KeyTree {
        KeyTree::node(
            "root",
            vec![
                (key('q'), KeyTree::Leaf(vec![Action::Quit])),
                (
                    key('g'),
                    KeyTree::node("goto", vec![(key('s'), KeyTree::Leaf(vec![Action::Save]))]),
                ),
            ],
        )
    }

    #[test]
    fn walk_reaches_a_leaf() {
        let tree = tree();
        assert!(matches!(
            tree.get(&[key('g'), key('s')]),
            Some(KeyTree::Leaf(actions)) if matches!(actions[..], [Action::Save])
        ));
    }

    #[test]
    fn walk_stops_on_a_node() {
        let tree = tree();
        assert!(
            matches!(tree.get(&[key('g')]), Some(KeyTree::Node { description, .. }) if description == "goto"),
            "a prefix waits for the next key"
        );
    }

    #[test]
    fn unbound_key_is_none() {
        let tree = tree();
        assert!(tree.get(&[key('x')]).is_none());
        assert!(tree.get(&[key('g'), key('x')]).is_none());
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
        assert!(tree.get(&[control_q]).is_none());
    }
}
