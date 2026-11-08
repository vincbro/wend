use std::path::PathBuf;

use wend_core::{
    buffer::Buffer,
    editor::Editor,
    frame::{Area, CellPosition, Frame},
    input::{
        Input, Mode,
        key::{Key, KeyCode},
    },
    output::Output,
    syntax::{Highlight, LanguageTag},
    view::{Direction, Motion},
};

fn editor(text: &str, cols: u16, rows: u16) -> Editor {
    let mut editor = Editor::new(Area::new(cols, rows)).expect("the wasm engine starts");
    editor.view_mut().open_buffer(Buffer::from(text));
    editor
}

/// The columns the line numbers take in front of the text, for a file that is short: three
/// digits and a space.
const GUTTER: u16 = 4;

/// What a row shows to the right of the gutter.
fn row_text(frame: &Frame, row: u16) -> String {
    let text: String = frame
        .row(row)
        .iter()
        .skip(usize::from(GUTTER))
        .filter(|cell| !cell.covered)
        .map(|cell| cell.grapheme.as_str_sanitize())
        .collect();
    text.trim_end().to_owned()
}

/// What a row shows in the gutter, the line numbers in front of the text.
fn gutter_text(frame: &Frame, row: u16) -> String {
    frame
        .row(row)
        .iter()
        .take(usize::from(GUTTER))
        .map(|cell| cell.grapheme.as_str_sanitize())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_inserts_in_front_of_the_cursor() {
        let mut editor = editor("world", 20, 3);
        editor.set_mode(Mode::Insert);
        for c in "hello ".chars() {
            editor.view_mut().insert(c);
        }

        let frame = editor.frame();
        assert_eq!(row_text(&frame, 0), "hello world");
        assert_eq!(
            frame.cursor(),
            Some(CellPosition::new(GUTTER + 6, 0)),
            "the cursor stays on the w it started on"
        );
    }

    #[test]
    fn motions_move_the_frame_cursor() {
        let mut editor = editor("one two\nthree", 20, 3);
        editor
            .view_mut()
            .move_selection(Motion::Vertical(Direction::Forward), false);
        editor
            .view_mut()
            .move_selection(Motion::Horizontal(Direction::Forward), false);

        assert_eq!(
            editor.frame().cursor(),
            Some(CellPosition::new(GUTTER + 1, 1)),
            "on the h of three"
        );
    }

    #[test]
    fn the_gutter_counts_rows_from_the_cursor() {
        let mut editor = editor("a\nb\nc\nd", 20, 4);
        for _ in 0..2 {
            editor
                .view_mut()
                .move_selection(Motion::Vertical(Direction::Forward), false);
        }

        let frame = editor.frame();
        let shown: Vec<_> = (0..4).map(|row| gutter_text(&frame, row)).collect();
        assert_eq!(
            shown,
            ["  2 ", "  1 ", "  3 ", "  1 "],
            "the cursor is on line 3, the other rows are their distance away"
        );
    }

    #[test]
    fn deleting_a_word_removes_it_from_the_frame() {
        let mut editor = editor("hello world", 20, 3);
        editor
            .view_mut()
            .move_selection(Motion::Word(Direction::Forward), false);
        editor.view_mut().delete_selection();

        let frame = editor.frame();
        assert_eq!(
            row_text(&frame, 0),
            "world",
            "the word and its space are gone"
        );
        assert_eq!(frame.cursor(), Some(CellPosition::new(GUTTER, 0)));
    }

    #[test]
    fn revert_shows_the_clean_text_again() {
        let mut editor = editor("abc", 20, 3);
        editor.view_mut().insert('x');
        editor.view_mut().revert();

        assert_eq!(row_text(&editor.frame(), 0), "abc");
    }

    #[test]
    fn jumping_to_the_end_scrolls_it_on_screen() {
        let text: Vec<String> = (0..50).map(|line| line.to_string()).collect();
        let mut editor = editor(&text.join("\n"), 20, 5);
        editor
            .view_mut()
            .move_selection(Motion::File(Direction::Forward), false);

        let frame = editor.frame();
        let cursor = frame.cursor().unwrap();
        assert!(cursor.row < 5, "the cursor is inside the frame");
        assert_eq!(
            row_text(&frame, cursor.row),
            "49",
            "the cursor's row shows the last line"
        );
        assert_eq!(cursor.col, GUTTER + 1, "on the last grapheme, the 9");
    }

    #[test]
    fn a_terminal_with_no_size_has_no_cursor() {
        for (cols, rows) in [(0, 0), (20, 0), (0, 5)] {
            let mut editor = editor("one\ntwo", cols, rows);
            let view = editor.view_mut();
            view.move_selection(Motion::Page(Direction::Forward), false);
            view.move_selection(Motion::Page(Direction::Backward), false);
            assert_eq!(
                editor.frame().cursor(),
                None,
                "nowhere to show it, {cols}x{rows}"
            );
        }
    }

    #[test]
    fn resizing_to_no_size_and_back_restores_the_cursor() {
        let mut editor = editor("abc", 20, 3);
        editor.resize(Area::new(0, 0));
        assert_eq!(editor.frame().cursor(), None, "nowhere to show it");

        editor.resize(Area::new(20, 3));
        assert_eq!(
            editor.frame().cursor(),
            Some(CellPosition::new(GUTTER, 0)),
            "back on the first grapheme"
        );
    }

    /// Applies `input` and returns the frames it sent, the way a frontend sees them.
    fn send(editor: &mut Editor, input: Input) -> Vec<Frame> {
        editor.apply(input).unwrap();
        editor
            .drain_outputs()
            .filter_map(|output| match output {
                Output::Frame(frame) => Some(frame),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn keys_in_frame_out() {
        let mut editor = Editor::new(Area::new(20, 3)).unwrap();
        let file = Input::File {
            path: PathBuf::from("a.txt"),
            bytes: b"abc".as_slice().into(),
            language: None,
        };
        let opened = send(&mut editor, file);
        assert_eq!(
            row_text(&opened[0], 0),
            "abc",
            "the file shows once it arrives"
        );
        send(
            &mut editor,
            Input::Drawn(opened.into_iter().next().unwrap()),
        );

        send(&mut editor, Input::Key(Key::new(KeyCode::Char('i'))));
        let typed = send(&mut editor, Input::Key(Key::new(KeyCode::Char('x'))));
        assert_eq!(
            row_text(typed.last().unwrap(), 0),
            "xabc",
            "typed in front of the cursor"
        );
    }

    #[test]
    fn highlights_reach_the_frame_once_the_grammar_loads() {
        let mut editor = Editor::new(Area::new(20, 3)).unwrap();
        let file = Input::File {
            path: PathBuf::from("a.rs"),
            bytes: b"fn main() {}".as_slice().into(),
            language: Some(LanguageTag::new("rust")),
        };
        let plain = send(&mut editor, file);
        assert_eq!(
            plain[0].get(CellPosition::new(GUTTER, 0)).highlight,
            None,
            "no colors until the grammar is there"
        );

        let loaded = send(
            &mut editor,
            Input::Language {
                tag: LanguageTag::new("rust"),
                wasm: include_bytes!("../../../tree-sitter-rust.wasm")
                    .as_slice()
                    .into(),
                highlights: include_bytes!("../../../highlights.scm").as_slice().into(),
            },
        );

        let highlight = |col: u16| loaded[0].get(CellPosition::new(GUTTER + col, 0)).highlight;
        assert_eq!(highlight(0), Some(Highlight::Keyword), "the f of fn");
        assert_eq!(highlight(3), Some(Highlight::Function), "the m of main");
    }

    fn rust_file() -> Input {
        Input::File {
            path: PathBuf::from("a.rs"),
            bytes: b"fn main() {}".as_slice().into(),
            language: Some(LanguageTag::new("rust")),
        }
    }

    #[test]
    fn the_grammar_is_asked_for_once() {
        let mut editor = Editor::new(Area::new(20, 3)).expect("the wasm engine starts");
        editor.apply(rust_file()).unwrap();
        for c in ['i', 'x', 'y'] {
            editor
                .apply(Input::Key(Key::new(KeyCode::Char(c))))
                .unwrap();
        }

        let requests = editor
            .drain_outputs()
            .filter(|output| matches!(output, Output::Language(_)))
            .count();

        assert_eq!(
            requests, 1,
            "typing while the grammar is on its way doesn't ask again"
        );
    }

    #[test]
    fn a_grammar_that_fails_to_load_leaves_the_text_plain() {
        let mut editor = Editor::new(Area::new(20, 3)).expect("the wasm engine starts");
        let opened = send(&mut editor, rust_file());
        send(
            &mut editor,
            Input::Drawn(opened.into_iter().next().unwrap()),
        );

        let frames = send(
            &mut editor,
            Input::Language {
                tag: LanguageTag::new("rust"),
                wasm: b"not wasm".as_slice().into(),
                highlights: b"".as_slice().into(),
            },
        );

        assert_eq!(
            row_text(&frames[0], 0),
            "fn main() {}",
            "the text is still there"
        );
        assert_eq!(
            frames[0].get(CellPosition::new(GUTTER, 0)).highlight,
            None,
            "without a grammar it is plain text"
        );
        send(
            &mut editor,
            Input::Drawn(frames.into_iter().next().unwrap()),
        );
        send(&mut editor, Input::Key(Key::new(KeyCode::Char('i'))));
        let typed = send(&mut editor, Input::Key(Key::new(KeyCode::Char('x'))));
        assert_eq!(
            row_text(typed.last().unwrap(), 0),
            "xfn main() {}",
            "the editor keeps taking input"
        );
    }

    fn rust_language() -> Input {
        Input::Language {
            tag: LanguageTag::new("rust"),
            wasm: include_bytes!("../../../tree-sitter-rust.wasm")
                .as_slice()
                .into(),
            highlights: include_bytes!("../../../highlights.scm").as_slice().into(),
        }
    }

    /// Applies `input` and hands the frames back the way a frontend does once they are drawn.
    /// Returns everything the editor sent.
    fn exchange(editor: &mut Editor, input: Input) -> Vec<Output> {
        editor.apply(input).unwrap();
        let outputs: Vec<Output> = editor.drain_outputs().collect();
        for output in &outputs {
            if let Output::Frame(frame) = output {
                editor.apply(Input::Drawn(frame.clone())).unwrap();
            }
        }
        outputs
    }

    #[test]
    fn a_second_file_of_a_loaded_language_is_highlighted_without_asking_again() {
        let mut editor = Editor::new(Area::new(20, 3)).expect("the wasm engine starts");
        exchange(&mut editor, rust_file());
        exchange(&mut editor, rust_language());

        let second = Input::File {
            path: PathBuf::from("b.rs"),
            bytes: b"fn other() {}".as_slice().into(),
            language: Some(LanguageTag::new("rust")),
        };
        let outputs = exchange(&mut editor, second);

        assert!(
            !outputs
                .iter()
                .any(|output| matches!(output, Output::Language(_))),
            "the grammar is already here, nobody needs to send it again"
        );
        let frame = outputs
            .into_iter()
            .find_map(|output| match output {
                Output::Frame(frame) => Some(frame),
                _ => None,
            })
            .expect("a frame is drawn for the new file");
        assert_eq!(
            frame.get(CellPosition::new(GUTTER, 0)).highlight,
            Some(Highlight::Keyword),
            "the fn of the second file"
        );
    }
}
