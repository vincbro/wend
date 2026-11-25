//! Drives a buffer with random calls, checks its invariants after each one (S-INVARIANTS)
//! and compares its text with a plain `String` that gets the same edits.

use proptest::{collection, prelude::*};

use super::{Buffer, Error};
use crate::{
    syntax::{LanguageTag, tests as syntax_tests},
    text_box::fuzz as text_box_fuzz,
};

/// One public call on a buffer, with its arguments. Edit ranges are fractions of the text
/// the buffer holds when the call runs, so they are always inside it.
#[derive(Debug, Clone)]
enum Call {
    Edit { at: usize, len: usize, text: String },
    Revert,
    Save,
    Update(Vec<u8>),
    SetLanguage(Option<&'static str>),
    Attach,
    Reparse,
}

fn call() -> impl Strategy<Value = Call> {
    let language = prop_oneof![Just(None), Just(Some("rust")), Just(Some("other"))];
    // Mostly text, sometimes bytes that aren't UTF-8
    let bytes = prop_oneof![
        4 => text_box_fuzz::text().prop_map(String::into_bytes),
        1 => collection::vec(any::<u8>(), 0..16),
    ];
    prop_oneof![
        8 => (any::<usize>(), any::<usize>(), text_box_fuzz::text())
            .prop_map(|(at, len, text)| Call::Edit { at, len, text }),
        1 => Just(Call::Revert),
        1 => Just(Call::Save),
        1 => bytes.prop_map(Call::Update),
        1 => language.prop_map(Call::SetLanguage),
        1 => Just(Call::Attach),
        1 => Just(Call::Reparse),
    ]
}

/// Moves `byte` back onto a char boundary of `text`.
fn snap(text: &str, mut byte: usize) -> usize {
    while !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

/// What the text of a buffer must be, kept by the plain strings that get the same calls.
struct Model {
    text: String,
    clean: String,
    is_dirty: bool,
}

impl Model {
    fn run(&mut self, call: Call, buffer: &mut Buffer) {
        match call {
            Call::Edit { at, len, text } => {
                let start = snap(&self.text, at % (self.text.len() + 1));
                let end = snap(&self.text, (start + len % 8).min(self.text.len()));
                buffer.edit(start..end, &text);
                self.text.replace_range(start..end, &text);
                self.is_dirty |= start != end || !text.is_empty();
            }
            Call::Revert => {
                buffer.revert();
                self.text.clone_from(&self.clean);
                self.is_dirty = false;
            }
            Call::Save => {
                let rope = buffer.save();
                assert_eq!(rope.to_string(), self.text, "save writes the text");
                self.clean.clone_from(&self.text);
                self.is_dirty = false;
            }
            Call::Update(bytes) => match str::from_utf8(&bytes) {
                Ok(new) => {
                    buffer.update(&bytes).unwrap();
                    self.clean = new.to_owned();
                    if !self.is_dirty {
                        self.text.clone_from(&self.clean);
                    }
                }
                Err(_) => assert!(
                    matches!(buffer.update(&bytes), Err(Error::InvalidUtf8)),
                    "bytes that aren't UTF-8 are refused"
                ),
            },
            Call::SetLanguage(tag) => buffer.set_language(tag.map(LanguageTag::new)),
            Call::Attach => syntax_tests::with_rust(|_, grammar| buffer.attach(grammar)),
            Call::Reparse => {
                syntax_tests::with_rust(|loader, _| buffer.reparse(loader).unwrap());
            }
        }
    }
}

proptest! {
    #[test]
    fn no_sequence_of_calls_breaks_the_invariants(
        initial in text_box_fuzz::text(),
        calls in collection::vec(call(), 0..40),
    ) {
        let mut buffer = Buffer::from_bytes(initial.as_bytes()).unwrap();
        let mut model = Model {
            text: initial.clone(),
            clean: initial,
            is_dirty: false,
        };

        for call in calls {
            model.run(call, &mut buffer);
            buffer.check_invariants();
            prop_assert_eq!(buffer.rope().to_string(), model.text.as_str(), "the text is what the edits made");
            prop_assert_eq!(
                buffer.is_dirty(),
                model.is_dirty,
                "dirty from the first edit until a revert or a save"
            );
        }
    }
}
