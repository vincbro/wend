//! Drives a keymap with random keys and modes and checks its invariants after each one
//! (S-INVARIANTS). A combo that the keymap hands back is compared with the leaf of the tree.

use proptest::{collection, prelude::*};

use super::Keymap;
use crate::{
    config::{Config, Target},
    input::{
        Mode,
        key::{Key, KeyTree},
    },
    text_box::fuzz::key,
};

fn mode() -> impl Strategy<Value = Mode> {
    prop_oneof![Just(Mode::Normal), Just(Mode::Select), Just(Mode::Insert)]
}

proptest! {
    #[test]
    fn no_sequence_of_keys_breaks_the_invariants(
        target in prop_oneof![Just(Target::TextBox), Just(Target::Scratch)],
        presses in collection::vec((mode(), key()), 0..60),
    ) {
        let config = Config::default();
        let mut keymap = Keymap::new(&config.keys, target).unwrap();
        let mut since_reset: Vec<Key> = vec![];
        let mut last_mode = Mode::Normal;
        for (mode, key) in presses {
            if mode != last_mode {
                since_reset.clear();
                last_mode = mode;
            }
            since_reset.push(key);
            let expected = match config.keys.trees(target).unwrap().get(mode).get(&since_reset) {
                Some(KeyTree::Leaf(actions)) => Some(actions.len()),
                _ => None,
            };
            let ran = keymap.press(mode, key).len();
            keymap.check_invariants();
            if let Some(count) = expected {
                prop_assert_eq!(ran, count, "a leaf hands back its actions");
                since_reset.clear();
            } else {
                prop_assert_eq!(ran, 0, "a node or an unbound key runs nothing");
                if keymap.pending.is_empty() {
                    since_reset.clear();
            }
            }
        }
    }
}
