//! Drives a text box with random calls and checks its invariants after each one (S-INVARIANTS).

use std::num::NonZeroUsize;

use proptest::{collection, prelude::*};

use super::{Direction, Motion, Selection, TextBox};
use crate::{
    buffer::Buffer,
    config::{Config, Target},
    frame::{Area, Rect},
    input::{
        Mode,
        action::TextBoxAction,
        key::{Key, KeyCode, Modifiers},
    },
};

/// One public call on a text box, with its arguments.
#[derive(Debug, Clone)]
enum Call {
    Perform(TextBoxAction),
    Resize(Rect),
    CursorPosition,
    Update(String),
    OpenBuffer(String),
    /// What `Motion::Match` looks for, and what a prompt searches from.
    Query(String, usize),
}

impl Call {
    /// Whether the call is the owner's, which a read-only box doesn't refuse.
    fn is_owners(&self) -> bool {
        matches!(
            self,
            Self::Update(_) | Self::OpenBuffer(_) | Self::Query(..)
        )
    }

    fn run(self, view: &mut TextBox) {
        match self {
            Self::Perform(action) => view.perform(action),
            Self::Resize(rect) => view.resize(rect),
            Self::CursorPosition => {
                view.cursor_position();
            }
            Self::Update(text) => view.update(text.as_bytes()).unwrap(),
            Self::Query(query, from) => {
                view.set_query(&query);
                let origin = Selection::point(from.min(view.buffer().rope().byte_len()));
                view.find_from(&query, view.selection());
                // The origin may be inside a grapheme, which the selection snaps
                view.restore_selection(origin);
            }
            Self::OpenBuffer(text) => {
                view.open_buffer(Buffer::from(text.as_str()));
            }
        }
    }
}

/// Keys the default bindings start combos with, others they leave unbound, and the named keys.
pub(crate) fn key() -> impl Strategy<Value = Key> {
    let code = prop_oneof![
        4 => prop::sample::select(vec!['g', 'h', 'j', 'k', 'l', 'x', ' ', 'z']).prop_map(KeyCode::Char),
        1 => any::<char>().prop_map(KeyCode::Char),
        1 => Just(KeyCode::Enter),
        1 => Just(KeyCode::Esc),
        1 => Just(KeyCode::Backspace),
        1 => Just(KeyCode::Tab),
    ];
    (code, any::<bool>(), any::<bool>()).prop_map(|(code, control, alt)| Key {
        code,
        modifiers: Modifiers {
            alt,
            control,
            shift: false,
        },
    })
}

/// Text made of the pieces that break naive code: wide, zero-width and combining glyphs,
/// tabs, both line breaks and empty lines.
pub(crate) fn text() -> impl Strategy<Value = String> {
    let pieces = prop::sample::select(vec![
        "a",
        "b",
        " ",
        "\t",
        "\n",
        "\r\n",
        "\n\n",
        "é",
        "e\u{301}",
        "漢",
        "🦀",
        "👨‍👩‍👧",
        "\u{200b}",
        "word_word",
        "a line long enough to wrap around a narrow view, more than once",
    ]);
    collection::vec(pieces, 0..40).prop_map(|pieces| pieces.concat())
}

/// From no cells at all to wider than the text ever gets.
pub(crate) fn rect() -> impl Strategy<Value = Rect> {
    (0..=150u16, 0..=12u16).prop_map(|(cols, rows)| Rect::new(0, 0, Area::new(cols, rows)))
}

/// Any size a terminal reports, from no cells at all to a big one.
pub(crate) fn area() -> impl Strategy<Value = Area> {
    (0..=150u16, 0..=40u16).prop_map(|(cols, rows)| Area::new(cols, rows))
}

fn direction() -> impl Strategy<Value = Direction> {
    prop_oneof![Just(Direction::Backward), Just(Direction::Forward)]
}

fn motion() -> impl Strategy<Value = Motion> {
    prop_oneof![
        direction().prop_map(Motion::Horizontal),
        direction().prop_map(Motion::Vertical),
        direction().prop_map(Motion::Word),
        direction().prop_map(Motion::Line),
        direction().prop_map(Motion::Page),
        direction().prop_map(Motion::File),
        direction().prop_map(Motion::Match),
    ]
}

fn mode() -> impl Strategy<Value = Mode> {
    prop_oneof![Just(Mode::Normal), Just(Mode::Select), Just(Mode::Insert)]
}

/// Any action a text box performs, with its arguments.
pub(crate) fn text_box_action() -> impl Strategy<Value = TextBoxAction> {
    prop_oneof![
        motion().prop_map(TextBoxAction::Motion),
        mode().prop_map(TextBoxAction::Mode),
        any::<char>().prop_map(TextBoxAction::Insert),
        Just(TextBoxAction::Delete),
        Just(TextBoxAction::DeleteSelection),
        Just(TextBoxAction::OpenLine),
        Just(TextBoxAction::CollapseSelection),
        Just(TextBoxAction::SelectRow),
        Just(TextBoxAction::Revert),
        Just(TextBoxAction::Find),
    ]
}

fn call() -> impl Strategy<Value = Call> {
    prop_oneof![
        8 => text_box_action().prop_map(Call::Perform),
        2 => rect().prop_map(Call::Resize),
        1 => Just(Call::CursorPosition),
        1 => text().prop_map(Call::Update),
        1 => text().prop_map(Call::OpenBuffer),
        2 => (prop::sample::select(vec!["a", "b", "é", "ab", "漢", "A", " ", "word", ""]), 0..80usize)
            .prop_map(|(query, from)| Call::Query(query.to_owned(), from)),
    ]
}

proptest! {
    #[test]
    fn no_sequence_of_calls_breaks_the_invariants(
        start in rect(),
        initial in text(),
        line_limit in prop::option::of(1..4usize),
        read_only in any::<bool>(),
        calls in collection::vec(call(), 0..40),
    ) {
        let mut view = TextBox::new(start, &Config::default(), Target::TextBox).unwrap();
        if let Some(limit) = line_limit {
            view.limit_lines(NonZeroUsize::new(limit).unwrap());
        }
        view.open_buffer(Buffer::from(initial.as_str()));
        view.set_read_only(read_only);
        view.check_invariants();

        // What only the owner changes, when the box is read-only
        let mut owned = view.buffer().rope().to_string();
        for call in calls {
            let by_owner = call.is_owners();
            call.run(&mut view);
            view.check_invariants();
            if read_only && !by_owner {
                prop_assert_eq!(&view.buffer().rope().to_string(), &owned);
            } else {
                owned = view.buffer().rope().to_string();
            }
        }
    }
}
