use crate::{
    config::{Actions, Bindings, Chord},
    input::{Mode, action::Action},
    view::{Direction, Motion},
};

fn bind(chord: &str, actions: &[Action]) -> (Chord, Actions) {
    let chord = chord
        .parse()
        .expect("every default chord is written in valid notation");
    (chord, Actions(actions.into()))
}

fn motion(chord: &str, motion: Motion) -> (Chord, Actions) {
    bind(chord, &[Action::Motion(motion)])
}

/// Normal and select mode share their bindings, select only makes motions extend. `v`
/// switches to `other_mode`.
pub(super) fn command_bindings(other_mode: Mode) -> Bindings {
    Bindings::from([
        motion("h", Motion::Horizontal(Direction::Backward)),
        motion("l", Motion::Horizontal(Direction::Forward)),
        motion("j", Motion::Vertical(Direction::Forward)),
        motion("k", Motion::Vertical(Direction::Backward)),
        motion("w", Motion::Word(Direction::Forward)),
        motion("b", Motion::Word(Direction::Backward)),
        motion("H", Motion::Line(Direction::Backward)),
        motion("L", Motion::Line(Direction::Forward)),
        motion("J", Motion::Page(Direction::Forward)),
        motion("K", Motion::Page(Direction::Backward)),
        motion("gs", Motion::File(Direction::Backward)),
        motion("ge", Motion::File(Direction::Forward)),
        bind("d", &[Action::DeleteSelection]),
        bind("u", &[Action::Revert]),
        bind("s", &[Action::Save]),
        bind("i", &[Action::Mode(Mode::Insert)]),
        bind("o", &[Action::OpenLine, Action::Mode(Mode::Insert)]),
        bind("v", &[Action::Mode(other_mode)]),
        bind("q", &[Action::Quit]),
        bind(
            "<esc>",
            &[Action::CollapseSelection, Action::Mode(Mode::Normal)],
        ),
    ])
}

/// Only the keys that don't type themselves: the editor types any character the bindings
/// don't bind.
pub(super) fn insert_bindings() -> Bindings {
    Bindings::from([
        bind("<esc>", &[Action::Mode(Mode::Normal)]),
        bind("<backspace>", &[Action::Delete]),
        bind("<enter>", &[Action::Insert('\n')]),
        bind("<tab>", &[Action::Insert('\t')]),
    ])
}
