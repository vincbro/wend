//! Drives a scratch with random calls and checks its invariants after each one (S-INVARIANTS).

use proptest::{collection, prelude::*};

use super::Scratch;
use crate::{
    buffer::Buffer,
    config::Config,
    frame::Rect,
    input::action::TextBoxAction,
    text_box::fuzz::{rect, text, text_box_action},
};

/// One public call on a scratch, with its arguments.
#[derive(Debug, Clone)]
enum Call {
    Perform(TextBoxAction),
    Resize(Rect),
    CursorPosition,
    Update(String),
    OpenBuffer(String),
    /// Sets a config whose line numbers have at least this many digits.
    SetMinDigits(usize),
}

impl Call {
    fn run(self, scratch: &mut Scratch) {
        match self {
            Self::Perform(action) => scratch.perform(action),
            Self::Resize(rect) => scratch.resize(rect),
            Self::CursorPosition => {
                scratch.cursor_position();
            }
            Self::Update(text) => scratch.update(text.as_bytes()).unwrap(),
            Self::OpenBuffer(text) => {
                scratch.open_buffer(Buffer::from(text.as_str()));
            }
            Self::SetMinDigits(min_digits) => {
                let mut config = Config::default();
                config.line_numbers.min_digits = min_digits;
                scratch.set_config(&config).unwrap();
            }
        }
    }
}

fn call() -> impl Strategy<Value = Call> {
    prop_oneof![
        8 => text_box_action().prop_map(Call::Perform),
        2 => rect().prop_map(Call::Resize),
        1 => Just(Call::CursorPosition),
        1 => text().prop_map(Call::Update),
        1 => text().prop_map(Call::OpenBuffer),
        1 => (0..=6usize).prop_map(Call::SetMinDigits),
    ]
}

proptest! {
    #[test]
    fn no_sequence_of_calls_breaks_the_invariants(
        start in rect(),
        initial in text(),
        calls in collection::vec(call(), 0..40),
    ) {
        let mut scratch = Scratch::new(start, &Config::default()).unwrap();
        scratch.open_buffer(Buffer::from(initial.as_str()));

        for call in calls {
            call.run(&mut scratch);
            scratch.check_invariants();
        }
    }
}
