//! Drives an overlay keymap with random keys and checks its invariants after each one
//! (S-INVARIANTS).

use proptest::{collection, prelude::*};

use super::Keymap;
use crate::{
    config::Config,
    input::{action::OverlayKind, key::KeyTree},
    text_box::fuzz::key,
};

proptest! {
    #[test]
    fn no_sequence_of_keys_breaks_the_invariants(
        kind in prop_oneof![Just(OverlayKind::Test), Just(OverlayKind::Command), Just(OverlayKind::Search), Just(OverlayKind::Find)],
        keys in collection::vec(key(), 0..60),
    ) {
        let tree = Config::default().keys.overlay_tree(kind).unwrap();
        let mut keymap = Keymap::new(tree.clone());
        let mut since_reset = vec![];
        for key in keys {
            since_reset.push(key);
            let expected = match tree.get(&since_reset) {
                Some(KeyTree::Leaf(actions)) => Some(actions.len()),
                _ => None,
            };
            let ran = keymap.press(key).len();
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
