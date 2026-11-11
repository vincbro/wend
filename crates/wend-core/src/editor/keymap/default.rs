use crate::{
    input::{
        Mode,
        action::Action,
        key::{Key, KeyCode, KeyTree},
    },
    view::{Direction, Motion},
};

pub(super) fn key(c: char) -> Key {
    Key::new(KeyCode::Char(c))
}

fn leaf(action: Action) -> KeyTree {
    KeyTree::Leaf(Box::new([action]))
}

fn motion_leaf(motion: Motion) -> KeyTree {
    leaf(Action::Motion(motion))
}

/// Normal and select mode share one tree, select only makes motions extend. `v` switches
/// to `other_mode`.
pub(super) fn command_tree(description: &str, other_mode: Mode) -> KeyTree {
    KeyTree::from_children(
        description,
        vec![
            (
                key('h'),
                motion_leaf(Motion::Horizontal(Direction::Backward)),
            ),
            (
                key('l'),
                motion_leaf(Motion::Horizontal(Direction::Forward)),
            ),
            (key('j'), motion_leaf(Motion::Vertical(Direction::Forward))),
            (key('k'), motion_leaf(Motion::Vertical(Direction::Backward))),
            (key('w'), motion_leaf(Motion::Word(Direction::Forward))),
            (key('b'), motion_leaf(Motion::Word(Direction::Backward))),
            (key('H'), motion_leaf(Motion::Line(Direction::Backward))),
            (key('L'), motion_leaf(Motion::Line(Direction::Forward))),
            (key('J'), motion_leaf(Motion::Page(Direction::Forward))),
            (key('K'), motion_leaf(Motion::Page(Direction::Backward))),
            (
                key('g'),
                KeyTree::from_children(
                    "goto",
                    vec![
                        (key('s'), motion_leaf(Motion::File(Direction::Backward))),
                        (key('e'), motion_leaf(Motion::File(Direction::Forward))),
                    ],
                ),
            ),
            (key('d'), leaf(Action::DeleteSelection)),
            (key('u'), leaf(Action::Revert)),
            (key('s'), leaf(Action::Save)),
            (key('i'), leaf(Action::Mode(Mode::Insert))),
            (
                key('o'),
                KeyTree::Leaf(Box::new([Action::OpenLine, Action::Mode(Mode::Insert)])),
            ),
            (key('v'), leaf(Action::Mode(other_mode))),
            (key('q'), leaf(Action::Quit)),
            (Key::new(KeyCode::Esc), leaf(Action::Mode(Mode::Normal))),
        ],
    )
}

/// Only the keys that don't type themselves: the editor types any character the tree
/// doesn't bind.
pub(super) fn insert_tree() -> KeyTree {
    KeyTree::from_children(
        "insert",
        vec![
            (Key::new(KeyCode::Esc), leaf(Action::Mode(Mode::Normal))),
            (Key::new(KeyCode::Backspace), leaf(Action::Delete)),
            (Key::new(KeyCode::Enter), leaf(Action::Insert('\n'))),
            (Key::new(KeyCode::Tab), leaf(Action::Insert('\t'))),
        ],
    )
}
