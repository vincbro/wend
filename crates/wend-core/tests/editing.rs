use std::path::Path;

use wend_core::{
    config::Config,
    editor::Editor,
    frame::{Area, CellPosition, Content, Frame},
    input::{
        Failure, Input, Mode,
        key::{Key, KeyCode},
    },
    output::Output,
    syntax::{Highlight, LanguageTag},
};

/// Applies `input` and hands the frames back the way a frontend does once they are drawn.
/// Returns everything the editor sent.
fn exchange(editor: &mut Editor, input: Input) -> Vec<Output> {
    editor.apply(input);
    let outputs: Vec<Output> = editor.drain_outputs().collect();
    for output in &outputs {
        if let Output::Frame(frame) = output {
            editor.apply(Input::Drawn(frame.clone()));
        }
    }
    outputs
}

/// The last frame among `outputs`, which is what the screen shows once they are all drawn.
fn last_frame(outputs: Vec<Output>) -> Frame {
    outputs
        .into_iter()
        .rev()
        .find_map(|output| match output {
            Output::Frame(frame) => Some(frame),
            _ => None,
        })
        .expect("every input is answered with a frame, because they are all handed back")
}

/// Presses `keys` one after another, `\u{1b}` being escape and `\n` enter, and returns the
/// last frame the editor drew.
fn press(editor: &mut Editor, keys: &str) -> Frame {
    let mut frame = None;
    for c in keys.chars() {
        let code = match c {
            '\u{1b}' => KeyCode::Esc,
            '\n' => KeyCode::Enter,
            _ => KeyCode::Char(c),
        };
        frame = Some(last_frame(exchange(editor, Input::Key(Key::new(code)))));
    }
    frame.expect("there is at least one key to press")
}

/// The rows under the view that the status bar takes.
const STATUS_ROWS: u16 = 1;

/// An editor whose view is `cols` by `rows`, with a status bar under it, that has opened a
/// plain text file holding `text`.
fn editor(text: &str, cols: u16, rows: u16) -> Editor {
    let mut editor = Editor::new(Area::new(cols, rows + STATUS_ROWS), &Config::default())
        .expect("the wasm engine starts");
    let file = Input::File {
        path: Path::new("a.txt").into(),
        bytes: text.as_bytes().into(),
        language: None,
    };
    exchange(&mut editor, file);
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
        let frame = press(&mut editor, "ihello ");

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
        let frame = press(&mut editor, "jl");

        assert_eq!(
            frame.cursor(),
            Some(CellPosition::new(GUTTER + 1, 1)),
            "on the h of three"
        );
    }

    #[test]
    fn the_gutter_counts_rows_from_the_cursor() {
        let mut editor = editor("a\nb\nc\nd", 20, 4);
        let frame = press(&mut editor, "jj");

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
        let frame = press(&mut editor, "wd");

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
        let frame = press(&mut editor, "ix\u{1b}u");

        assert_eq!(row_text(&frame, 0), "abc");
    }

    #[test]
    fn jumping_to_the_end_scrolls_it_on_screen() {
        let text: Vec<String> = (0..50).map(|line| line.to_string()).collect();
        let mut editor = editor(&text.join("\n"), 20, 5);
        let frame = press(&mut editor, "ge");

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
    fn the_status_bar_is_the_last_row() {
        let mut editor = editor("abc", 20, 3);
        let frame = press(&mut editor, "l");

        let last: String = frame
            .row(3)
            .iter()
            .map(|cell| cell.grapheme.as_str_sanitize())
            .collect();
        assert_eq!(last.trim_end(), "nor", "the mode, under the scratch");
        assert_eq!(
            row_text(&frame, 2),
            "",
            "and the scratch's own rows above it"
        );
    }

    #[test]
    fn a_terminal_wider_than_the_text_centers_it() {
        // The gutter and 100 columns of text are 104 wide, so 96 columns are left over
        let mut editor = editor("abc", 200, 3);
        let frame = press(&mut editor, "l");

        let col = |col| {
            frame
                .get(CellPosition::new(col, 0))
                .grapheme
                .as_str_sanitize()
        };
        assert_eq!(col(47), " ", "blank left of the gutter");
        assert_eq!(
            col(52),
            "a",
            "the text starts after 48 columns and the gutter"
        );
        assert_eq!(
            frame.cursor(),
            Some(CellPosition::new(53, 0)),
            "the cursor moved to the b, in the centered text"
        );
    }

    #[test]
    fn a_terminal_with_no_size_has_no_cursor() {
        for (cols, rows) in [(0, 0), (20, 0), (0, 5)] {
            let mut editor = editor("one\ntwo", cols, rows);
            let frame = press(&mut editor, "JK");
            assert_eq!(frame.cursor(), None, "nowhere to show it, {cols}x{rows}");
        }
    }

    #[test]
    fn resizing_to_no_size_and_back_restores_the_cursor() {
        let mut editor = editor("abc", 20, 3);
        let empty = last_frame(exchange(&mut editor, Input::Resize(Area::new(0, 0))));
        assert_eq!(empty.cursor(), None, "nowhere to show it");

        let restored = last_frame(exchange(
            &mut editor,
            Input::Resize(Area::new(20, 3 + STATUS_ROWS)),
        ));
        assert_eq!(
            restored.cursor(),
            Some(CellPosition::new(GUTTER, 0)),
            "back on the first grapheme"
        );
    }

    /// Applies `input` and returns the frames it sent, the way a frontend sees them.
    fn send(editor: &mut Editor, input: Input) -> Vec<Frame> {
        editor.apply(input);
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
        let mut editor = Editor::new(Area::new(20, 3), &Config::default()).unwrap();
        let file = Input::File {
            path: Path::new("a.txt").into(),
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
        let mut editor = Editor::new(Area::new(20, 3), &Config::default()).unwrap();
        let file = Input::File {
            path: Path::new("a.rs").into(),
            bytes: b"fn main() {}".as_slice().into(),
            language: Some(LanguageTag::new("rust")),
        };
        let plain = send(&mut editor, file);
        assert_eq!(
            plain[0].get(CellPosition::new(GUTTER, 0)).style.content,
            Content::Plain,
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

        let content = |col: u16| {
            loaded[0]
                .get(CellPosition::new(GUTTER + col, 0))
                .style
                .content
        };
        assert_eq!(content(0), Content::Text(Highlight::Keyword), "the f of fn");
        assert_eq!(
            content(3),
            Content::Text(Highlight::Function),
            "the m of main"
        );
    }

    fn rust_file() -> Input {
        Input::File {
            path: Path::new("a.rs").into(),
            bytes: b"fn main() {}".as_slice().into(),
            language: Some(LanguageTag::new("rust")),
        }
    }

    #[test]
    fn the_grammar_is_asked_for_once() {
        let mut editor =
            Editor::new(Area::new(20, 3), &Config::default()).expect("the wasm engine starts");
        editor.apply(rust_file());
        for c in ['i', 'x', 'y'] {
            editor.apply(Input::Key(Key::new(KeyCode::Char(c))));
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
        let mut editor =
            Editor::new(Area::new(20, 3), &Config::default()).expect("the wasm engine starts");
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
            frames[0].get(CellPosition::new(GUTTER, 0)).style.content,
            Content::Plain,
            "without a grammar it is plain text"
        );
        assert!(
            editor
                .message()
                .is_some_and(|message| message.starts_with("syntax failed")),
            "and the user is told why"
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

    #[test]
    fn a_second_file_of_a_loaded_language_is_highlighted_without_asking_again() {
        let mut editor =
            Editor::new(Area::new(20, 3), &Config::default()).expect("the wasm engine starts");
        exchange(&mut editor, rust_file());
        exchange(&mut editor, rust_language());

        let second = Input::File {
            path: Path::new("b.rs").into(),
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
            frame.get(CellPosition::new(GUTTER, 0)).style.content,
            Content::Text(Highlight::Keyword),
            "the fn of the second file"
        );
    }

    #[test]
    fn leader_e_opens_two_windows_that_take_the_keys_until_escape() {
        let mut editor = editor("abc", 100, 20);

        let frame = press(&mut editor, " e");
        let window_row = (0..21)
            .map(|row| row_text(&frame, row))
            .find(|row| row.contains("left window"))
            .expect("the left window is drawn");
        assert!(
            window_row.contains("right window"),
            "the right window is next to it: {window_row}"
        );

        let frame = press(&mut editor, "ihello");
        assert_eq!(row_text(&frame, 0), "abc", "no key was typed into the text");
        assert_eq!(editor.mode(), Mode::Normal, "and not to the editor's mode");

        let frame = press(&mut editor, "\u{1b}");
        assert_eq!(row_text(&frame, 0), "abc", "the editor is back as it was");
        assert!(
            (0..21).all(|row| !row_text(&frame, row).contains("window")),
            "and the windows are gone"
        );
        let frame = press(&mut editor, "l");
        assert!(frame.cursor().is_some(), "the editor takes keys again");
    }

    /// Everything on the last row of `frame`, the status bar's, without the trailing blanks.
    fn bottom_row(frame: &Frame) -> String {
        let last = frame.area().rows - 1;
        let text: String = frame
            .row(last)
            .iter()
            .map(|cell| cell.grapheme.as_str_sanitize())
            .collect();
        text.trim_end().to_owned()
    }

    /// What is typed in the command line of `frame`, none when it isn't there.
    fn typed_command(frame: &Frame) -> Option<String> {
        (0..frame.area().rows).find_map(|row| {
            let text: String = frame
                .row(row)
                .iter()
                .map(|cell| cell.grapheme.as_str_sanitize())
                .collect();
            let (_, after) = text.split_once("│: ")?;
            Some(after.split('│').next()?.trim_end().to_owned())
        })
    }

    #[test]
    fn a_command_runs_the_action_it_names_after_closing_the_command_line() {
        let mut editor = editor("abc", 100, 20);
        press(&mut editor, "l");

        let frame = press(&mut editor, ":");
        assert_eq!(
            typed_command(&frame).as_deref(),
            Some(""),
            "the command line floats over the editor, empty"
        );
        assert_eq!(bottom_row(&frame), "nor", "and leaves the status bar alone");

        let frame = press(&mut editor, "left\n");
        assert_eq!(typed_command(&frame), None, "the command line is gone");
        assert_eq!(
            frame.cursor(),
            Some(CellPosition::new(frame.cursor().unwrap().col, 0)),
            "and the cursor is back in the text"
        );
    }

    #[test]
    fn a_command_moves_the_text_and_not_the_command_line() {
        let mut editor = editor("abc", 100, 20);
        let start = press(&mut editor, "l").cursor().unwrap();

        let frame = press(&mut editor, ":left\n");

        assert_eq!(
            frame.cursor().unwrap().col + 1,
            start.col,
            "the cursor of the text went one left"
        );
    }

    #[test]
    fn a_command_that_names_nothing_says_so_in_the_status_bar() {
        let mut editor = editor("abc", 100, 20);

        let frame = press(&mut editor, ":xyz\n");

        assert_eq!(bottom_row(&frame), "unknown action `xyz`");
        let frame = press(&mut editor, "l");
        assert_eq!(bottom_row(&frame), "nor", "the next key clears it");
    }

    #[test]
    fn the_command_line_opens_in_insert_mode_and_escape_closes_it_from_normal_mode() {
        let mut editor = editor("abc", 100, 20);

        let frame = press(&mut editor, ":a");
        assert_eq!(
            typed_command(&frame).as_deref(),
            Some("a"),
            "it types at once"
        );

        let frame = press(&mut editor, "\u{1b}o");
        assert_eq!(
            typed_command(&frame).as_deref(),
            Some("a"),
            "o opens no second line"
        );

        let frame = press(&mut editor, "\u{1b}");
        assert_eq!(
            typed_command(&frame).as_deref(),
            Some("a"),
            "esc in insert mode only leaves it"
        );
        let frame = press(&mut editor, "\u{1b}");
        assert_eq!(typed_command(&frame), None, "esc in normal mode closes it");
    }

    #[test]
    fn the_command_line_keeps_its_box_in_a_small_terminal() {
        let mut editor = editor("abc", 12, 3);

        let frame = press(&mut editor, ":ab");

        assert!(
            frame.cursor().is_some(),
            "the cursor is in the box, whatever room there is"
        );
    }

    #[test]
    fn a_save_that_has_nowhere_to_go_is_said_in_the_status_bar_and_the_editor_goes_on() {
        let mut editor = Editor::new(Area::new(40, 4), &Config::default()).unwrap();

        press(&mut editor, ":save\n");
        let frame = last_frame(exchange(
            &mut editor,
            Input::Key(Key::new(KeyCode::Char('l'))),
        ));
        assert_eq!(bottom_row(&frame), "nor", "the next key clears it");

        let frame = press(&mut editor, ":w\n");
        assert_eq!(
            bottom_row(&frame),
            "the file shown has no path to save to",
            "it is in the status bar"
        );
        let last = frame.area().rows - 1;
        assert_eq!(
            frame.get(CellPosition::new(0, last)).style.content,
            Content::Error,
            "in the style of an error"
        );
        let frame = press(&mut editor, "ia");
        assert_eq!(row_text(&frame, 0), "a", "and the editor takes keys");
    }

    #[test]
    fn what_the_frontend_failed_at_is_said_with_its_reason() {
        let mut editor = Editor::new(Area::new(80, 4), &Config::default()).unwrap();
        let failures = [
            (
                Failure::Read {
                    path: Path::new("a.txt").into(),
                    reason: "no such file".into(),
                },
                "failed to read a.txt: no such file",
            ),
            (
                Failure::Write {
                    path: Path::new("b.txt").into(),
                    reason: "disk full".into(),
                },
                "failed to write b.txt: disk full",
            ),
        ];

        for (failure, said) in failures {
            let frame = last_frame(exchange(&mut editor, Input::Failed(failure)));

            assert_eq!(bottom_row(&frame), said);
            assert_eq!(editor.message(), Some(said));
        }
    }

    #[test]
    fn a_grammar_that_never_comes_may_be_asked_for_again() {
        let mut editor = Editor::new(Area::new(40, 4), &Config::default()).unwrap();
        let asks = |outputs: Vec<Output>| {
            outputs
                .iter()
                .filter(|output| matches!(output, Output::Language(_)))
                .count()
        };
        assert_eq!(asks(exchange(&mut editor, rust_file())), 1, "asked once");
        assert_eq!(
            asks(exchange(&mut editor, rust_file_named("b.rs"))),
            0,
            "and not again while it is awaited"
        );

        let outputs = exchange(
            &mut editor,
            Input::Failed(Failure::Language {
                tag: LanguageTag::new("rust"),
                reason: "not found".into(),
            }),
        );
        assert_eq!(asks(outputs), 0);
        assert_eq!(editor.message(), Some("no grammar for rust: not found"));

        assert_eq!(
            asks(exchange(&mut editor, rust_file_named("c.rs"))),
            1,
            "it isn't awaited any more, so a new file asks"
        );
    }

    fn rust_file_named(name: &str) -> Input {
        Input::File {
            path: Path::new(name).into(),
            bytes: b"fn main() {}".as_slice().into(),
            language: Some(LanguageTag::new("rust")),
        }
    }
}
