//! Drives an editor the way a frontend does, with random inputs, and checks its invariants
//! after each one (S-INVARIANTS).

use std::{cell::RefCell, num::NonZeroUsize, path::Path};

use proptest::{collection, prelude::*};

use super::{Editor, FRAME_COUNT};
use crate::{
    config::Config,
    frame::{Area, Frame},
    input::{
        Input,
        action::{Action, EditorAction},
        key::{Key, KeyCode},
    },
    output::Output,
    syntax::{LanguageTag, Loader, tests as syntax_tests},
    view::fuzz as view_fuzz,
};

/// One thing a frontend can do to the editor, with its arguments.
#[derive(Debug, Clone)]
enum Call {
    Key(Key),
    Action(Action),
    Resize(Area),
    File {
        path: usize,
        bytes: Vec<u8>,
        language: Option<&'static str>,
    },
    /// Hands back the frame the frontend holds at this position, if it holds one.
    Drawn(usize),
    /// Sends bytes that aren't a grammar. Loading the real one takes about 450 ms, so the
    /// tests of the buffer and of `load_language` cover that.
    BadLanguage,
    Config {
        tab_width: usize,
        margin: usize,
        min_digits: usize,
        padding: u16,
    },
}

/// Paths to cycle through, so a file is opened again, and a language is asked for twice.
const PATHS: [&str; 3] = ["a.rs", "b.txt", "c.rs"];

fn call() -> impl Strategy<Value = Call> {
    // The characters the default keys bind, and a few they don't
    let chars = "abcdefghijklmnopqrstuvwxyz GHJKL漢é🦀";
    let code = prop_oneof![
        8 => prop::sample::select(chars.chars().collect::<Vec<_>>()).prop_map(KeyCode::Char),
        1 => Just(KeyCode::Enter),
        2 => Just(KeyCode::Esc),
        1 => Just(KeyCode::Backspace),
        1 => Just(KeyCode::Tab),
    ];
    let key = (code, any::<bool>(), any::<bool>()).prop_map(|(code, alt, control)| {
        let mut key = Key::new(code);
        key.modifiers.alt = alt;
        key.modifiers.control = control;
        key
    });
    let action = prop_oneof![
        4 => view_fuzz::view_action().prop_map(Action::View),
        1 => Just(Action::Editor(EditorAction::OpenTestOverlay)),
        1 => Just(Action::Editor(EditorAction::Save)),
    ];
    let area = (0..=150u16, 0..=40u16).prop_map(|(cols, rows)| Area::new(cols, rows));
    let bytes = prop_oneof![
        4 => view_fuzz::text().prop_map(String::into_bytes),
        1 => collection::vec(any::<u8>(), 0..16),
    ];
    let language = prop_oneof![Just(None), Just(Some("rust")), Just(Some("other"))];
    prop_oneof![
        12 => key.prop_map(Call::Key),
        4 => action.prop_map(Call::Action),
        2 => area.prop_map(Call::Resize),
        2 => (0..PATHS.len(), bytes, language)
            .prop_map(|(path, bytes, language)| Call::File { path, bytes, language }),
        4 => (0..FRAME_COUNT).prop_map(Call::Drawn),
        1 => Just(Call::BadLanguage),
        1 => (1..=8usize, 0..=5usize, 0..=6usize, 0..=3u16).prop_map(
            |(tab_width, margin, min_digits, padding)| Call::Config { tab_width, margin, min_digits, padding }
        ),
    ]
}

/// The editor, and the frames the frontend is drawing.
struct Frontend {
    editor: Editor,
    area: Area,
    held: Vec<Frame>,
}

impl Frontend {
    fn run(&mut self, call: Call) {
        let input = match call {
            Call::Key(key) => Input::Key(key),
            Call::Action(action) => Input::Action(action),
            Call::Resize(area) => {
                self.area = area;
                Input::Resize(area)
            }
            Call::File {
                path,
                bytes,
                language,
            } => Input::File {
                path: Path::new(PATHS[path]).into(),
                bytes: bytes.into(),
                language: language.map(LanguageTag::new),
            },
            Call::Drawn(index) => {
                if self.held.is_empty() {
                    return;
                }
                Input::Drawn(self.held.swap_remove(index % self.held.len()))
            }
            Call::BadLanguage => Input::Language {
                tag: LanguageTag::new("rust"),
                wasm: Box::from(&b"not a grammar"[..]),
                highlights: syntax_tests::RUST_HIGHLIGHTS.into(),
            },
            Call::Config {
                tab_width,
                margin,
                min_digits,
                padding,
            } => {
                let mut config = Config::default();
                config.text.tab_width = NonZeroUsize::new(tab_width).unwrap();
                config.scroll.margin = margin;
                config.line_numbers.min_digits = min_digits;
                config.line_numbers.padding = padding;
                Input::Config(Box::new(config))
            }
        };
        // What fails is the file or the grammar being bad, which the invariants outlive
        drop(self.editor.apply(input));
        self.collect_frames();
    }

    /// Takes the frames the editor sent out, the way the frontend would draw them.
    fn collect_frames(&mut self) {
        for output in self.editor.drain_outputs() {
            if let Output::Frame(frame) = output {
                assert_eq!(
                    frame.area(),
                    self.area,
                    "a frame sent out is as big as the terminal"
                );
                self.held.push(frame);
            }
        }
        assert!(
            self.held.len() <= FRAME_COUNT,
            "only the frames the pool made are ever out"
        );
    }
}

thread_local! {
    /// The loader of the editor the last case ran, for the next one. Starting a loader starts
    /// a wasm engine, which takes about 80 ms, and no case here loads a grammar for the next
    /// one to find.
    static LOADER: RefCell<Option<Loader>> = const { RefCell::new(None) };
}

/// Draws and lays out whole terminals, about 3 ms a case, so this test runs a quarter of the
/// cases the others do.
fn config() -> ProptestConfig {
    let default = ProptestConfig::default();
    ProptestConfig {
        cases: default.cases / 4 + 1,
        ..default
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn no_sequence_of_inputs_breaks_the_invariants(
        area in (0..=150u16, 0..=40u16).prop_map(|(cols, rows)| Area::new(cols, rows)),
        calls in collection::vec(call(), 0..40),
    ) {
        let loader = LOADER.take().unwrap_or_else(|| Loader::new().unwrap());
        let mut frontend = Frontend {
            editor: Editor::with_loader(area, &Config::default(), loader).unwrap(),
            area,
            held: vec![],
        };
        for call in calls {
            frontend.run(call);
            frontend.editor.check_invariants();
        }
        LOADER.set(Some(frontend.editor.loader));
    }
}
