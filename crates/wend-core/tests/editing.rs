use std::path::Path;

use wend_core::{
    config::Config,
    editor::Editor,
    frame::{Area, CellPosition, Content, Frame},
    input::{
        Failure, Input, Mode,
        action::{Action, EditorAction, SearchKind},
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

/// The search overlay, with a fake frontend that answers from a list of files.
#[cfg(test)]
mod search {
    use super::*;

    const FILES: [&str; 3] = ["src/main.rs", "src/lib.rs", "README.md"];

    /// Presses `keys` and returns everything the editor sent, frames included.
    fn keys(editor: &mut Editor, keys: &str) -> Vec<Output> {
        let mut all = vec![];
        for c in keys.chars() {
            let code = match c {
                '\u{1b}' => KeyCode::Esc,
                '\n' => KeyCode::Enter,
                _ => KeyCode::Char(c),
            };
            all.extend(exchange(editor, Input::Key(Key::new(code))));
        }
        all
    }

    /// The searches in `outputs`, as their id and query.
    fn asked(outputs: &[Output]) -> Vec<(u64, String)> {
        outputs
            .iter()
            .filter_map(|output| match output {
                Output::Search { id, query, .. } => Some((*id, query.to_string())),
                _ => None,
            })
            .collect()
    }

    /// The files `outputs` asked to read.
    fn read(outputs: &[Output]) -> Vec<String> {
        outputs
            .iter()
            .filter_map(|output| match output {
                Output::Read { path } => Some(path.display().to_string()),
                _ => None,
            })
            .collect()
    }

    /// What the frontend answers to a search: the files that hold the query.
    fn answer(editor: &mut Editor, id: u64, query: &str) -> Vec<Output> {
        let text: Vec<&str> = FILES
            .iter()
            .copied()
            .filter(|file| file.contains(query))
            .collect();
        exchange(
            editor,
            Input::SearchResults {
                id,
                text: text.join("\n").into(),
            },
        )
    }

    fn text(frame: &Frame) -> String {
        (0..frame.area().rows)
            .map(|row| {
                frame
                    .row(row)
                    .iter()
                    .map(|cell| cell.grapheme.as_str_sanitize())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn opened() -> Editor {
        let mut editor = editor("abc", 100, 20);
        let outputs = keys(&mut editor, " f");
        assert_eq!(
            asked(&outputs),
            [(1, String::new())],
            "an empty query first"
        );
        answer(&mut editor, 1, "");
        editor
    }

    #[test]
    fn the_results_are_shown_under_the_query() {
        let mut editor = opened();

        let frame = last_frame(answer(&mut editor, 1, ""));

        let shown = text(&frame);
        assert!(shown.contains("src/main.rs"), "the first result");
        assert!(shown.contains("README.md"), "and the last");
    }

    #[test]
    fn typing_asks_for_a_new_search_with_the_next_id() {
        let mut editor = opened();

        let outputs = keys(&mut editor, "rs");

        assert_eq!(
            asked(&outputs),
            [(2, "r".to_owned()), (3, "rs".to_owned())],
            "one search per change of the text"
        );
    }

    #[test]
    fn enter_opens_the_first_result_of_the_newest_search() {
        let mut editor = opened();
        let outputs = keys(&mut editor, "lib");
        let (id, query) = asked(&outputs).pop().unwrap();
        answer(&mut editor, id, &query);

        let outputs = keys(&mut editor, "\n");

        assert_eq!(read(&outputs), ["src/lib.rs"]);
        assert!(
            !text(&last_frame(outputs)).contains("README.md"),
            "the search is closed"
        );
    }

    #[test]
    fn an_answer_to_an_older_search_is_dropped() {
        let mut editor = opened();
        keys(&mut editor, "r");

        answer(&mut editor, 1, "");
        let outputs = keys(&mut editor, "\n");

        assert!(
            read(&outputs).is_empty(),
            "the results of search 1 are not the ones for `r`, so enter waits"
        );
    }

    #[test]
    fn enter_before_the_answer_waits_for_it_and_picks_the_first_result() {
        let mut editor = opened();
        keys(&mut editor, "md");
        let outputs = keys(&mut editor, "\n");
        assert!(read(&outputs).is_empty(), "nothing to pick from yet");

        let outputs = answer(&mut editor, 3, "md");

        assert_eq!(read(&outputs), ["README.md"]);
    }

    #[test]
    fn leaving_the_query_moves_to_the_results_and_enter_picks_the_row_the_cursor_is_on() {
        let mut editor = opened();

        let outputs = keys(&mut editor, "\u{1b}jj\n");

        assert_eq!(read(&outputs), ["README.md"], "two rows down");
    }

    #[test]
    fn the_results_refuse_edits_and_i_goes_back_to_the_query() {
        let mut editor = opened();
        keys(&mut editor, "\u{1b}");

        let outputs = keys(&mut editor, "dxu");
        assert!(asked(&outputs).is_empty(), "nothing typed, nothing asked");
        let outputs = keys(&mut editor, "i");
        assert_eq!(asked(&outputs).len(), 0, "i asks for nothing");
        let outputs = keys(&mut editor, "z");

        assert_eq!(
            asked(&outputs),
            [(2, "z".to_owned())],
            "typing is in the query again"
        );
        assert_eq!(
            editor.mode(),
            Mode::Normal,
            "the scratch under it never had the keys"
        );
    }

    #[test]
    fn the_text_search_asks_for_text_and_picking_opens_the_file_at_the_line() {
        let mut editor = editor("abc", 40, 10);

        let outputs = keys(&mut editor, " /");
        assert!(
            outputs.iter().any(|output| matches!(
                output,
                Output::Search {
                    id: 1,
                    kind: SearchKind::Text,
                    ..
                }
            )),
            "the text search is asked for"
        );
        exchange(
            &mut editor,
            Input::SearchResults {
                id: 1,
                text: "a.txt:3:three\nb.txt:1:x".into(),
            },
        );
        let outputs = keys(&mut editor, "\n");
        assert_eq!(read(&outputs), ["a.txt"], "the file of the result");

        let lines = "one\ntwo\nthree\nfour";
        let frame = last_frame(exchange(
            &mut editor,
            Input::File {
                path: Path::new("a.txt").into(),
                bytes: lines.as_bytes().into(),
                language: None,
            },
        ));

        assert_eq!(
            frame.cursor().expect("a cursor").row,
            2,
            "on the third line"
        );
    }

    #[test]
    fn escape_twice_closes_the_search() {
        let mut editor = opened();

        let frame = last_frame(keys(&mut editor, "\u{1b}\u{1b}"));

        assert!(!text(&frame).contains("src/main.rs"), "closed");
    }

    #[test]
    fn a_failed_search_is_said_in_the_status_bar_and_leaves_the_results() {
        let mut editor = opened();
        keys(&mut editor, "x");

        let frame = last_frame(exchange(
            &mut editor,
            Input::Failed(Failure::Search {
                id: 2,
                reason: "fzf: not found".into(),
            }),
        ));

        assert!(text(&frame).contains("fzf: not found"), "the reason");
        assert!(text(&frame).contains("src/main.rs"), "the results stay");
    }
}

/// Finding in the text with `/`, `n` and `p`.
#[cfg(test)]
mod find {
    use super::*;

    fn text(frame: &Frame) -> String {
        (0..frame.area().rows)
            .map(|row| {
                frame
                    .row(row)
                    .iter()
                    .map(|cell| cell.grapheme.as_str_sanitize())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The text the cursor's selection covers, read off the frame.
    fn selected(frame: &Frame) -> String {
        frame
            .iter()
            .filter(|(_, cell)| cell.style.selected)
            .map(|(_, cell)| cell.grapheme.as_str_sanitize())
            .collect()
    }

    #[test]
    fn typing_selects_the_first_match_and_enter_keeps_it() {
        let mut editor = editor("one two\nthree two", 40, 10);

        let frame = press(&mut editor, "/tw");
        assert_eq!(selected(&frame), "tw", "the match as it is typed");
        let frame = press(&mut editor, "o\n");

        assert_eq!(selected(&frame), "two");
        assert!(!text(&frame).contains("/ two"), "the prompt is closed");
    }

    #[test]
    fn escape_puts_the_selection_back() {
        let mut editor = editor("one two", 40, 10);
        press(&mut editor, "l");

        let frame = press(&mut editor, "/two\u{1b}");

        assert_eq!(selected(&frame), "n", "the cursor is where it was");
    }

    /// The column the cursor is on, past the gutter.
    fn column(frame: &Frame) -> u16 {
        frame.cursor().expect("a cursor").col - GUTTER
    }

    #[test]
    fn n_and_p_move_between_matches_and_wrap() {
        let mut editor = editor("x1 x2 x3", 40, 10);
        assert_eq!(column(&press(&mut editor, "/x\n")), 0, "the first match");

        assert_eq!(column(&press(&mut editor, "n")), 3);
        assert_eq!(column(&press(&mut editor, "n")), 6);
        assert_eq!(column(&press(&mut editor, "n")), 0, "wraps to the first");
        assert_eq!(column(&press(&mut editor, "p")), 6, "and back to the last");
    }

    #[test]
    fn d_deletes_the_match() {
        let mut editor = editor("one two three", 40, 10);

        let frame = press(&mut editor, "/two\nd");

        assert!(text(&frame).contains("one  three"), "{}", text(&frame));
    }

    #[test]
    fn a_missing_match_leaves_the_cursor_where_it_was() {
        let mut editor = editor("one two", 40, 10);

        let frame = press(&mut editor, "/zzz");

        assert_eq!(selected(&frame), "o");
    }

    fn numbered(count: usize) -> String {
        (0..count)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn a_match_off_screen_is_scrolled_to_the_middle() {
        let mut editor = editor(&numbered(100), 40, 20);

        let frame = press(&mut editor, "/line 50\n");

        assert_eq!(frame.cursor().expect("a cursor").row, 10, "the middle row");
        assert!(text(&frame).contains("line 50"));
    }

    #[test]
    fn a_match_already_in_view_stays_where_it_is() {
        let mut editor = editor(&numbered(100), 40, 20);

        let frame = press(&mut editor, "/line 3\n");

        assert_eq!(
            frame.cursor().expect("a cursor").row,
            3,
            "the text didn't move"
        );
    }

    #[test]
    fn a_match_under_the_prompt_is_moved_clear_of_it() {
        let mut editor = editor(&numbered(100), 40, 20);

        let frame = press(&mut editor, "/line 18");

        assert_eq!(
            text(&frame).matches("line 18").count(),
            2,
            "in the prompt, and in the text where the prompt doesn't cover it"
        );
    }

    #[test]
    fn escape_puts_the_scroll_back_with_the_selection() {
        let mut editor = editor(&numbered(100), 40, 20);

        let frame = press(&mut editor, "/line 50");
        assert!(text(&frame).contains("line 50"), "scrolled to the match");
        let frame = press(&mut editor, "\u{1b}");

        assert!(text(&frame).contains("line 0"), "back at the top");
        assert!(
            !text(&frame).contains("line 50"),
            "and not where the match was"
        );
    }

    #[test]
    fn open_at_puts_the_cursor_on_the_line_of_a_file_that_is_already_shown() {
        let mut editor = editor(&numbered(10), 40, 10);

        let outputs = exchange(
            &mut editor,
            Input::Action(Action::Editor(EditorAction::OpenAt {
                path: Path::new("a.txt").into(),
                line: 5,
            })),
        );
        assert!(
            outputs
                .iter()
                .any(|output| matches!(output, Output::Read { .. })),
            "the file is read again"
        );
        let frame = last_frame(exchange(
            &mut editor,
            Input::File {
                path: Path::new("a.txt").into(),
                bytes: numbered(10).as_bytes().into(),
                language: None,
            },
        ));

        assert_eq!(frame.cursor().expect("a cursor").row, 4, "the fifth line");
    }

    #[test]
    fn a_file_that_fails_to_read_leaves_no_line_to_go_to() {
        let mut editor = editor("abc", 40, 10);
        exchange(
            &mut editor,
            Input::Action(Action::Editor(EditorAction::OpenAt {
                path: Path::new("b.txt").into(),
                line: 5,
            })),
        );
        exchange(
            &mut editor,
            Input::Failed(Failure::Read {
                path: Path::new("b.txt").into(),
                reason: "gone".into(),
            }),
        );

        let frame = last_frame(exchange(
            &mut editor,
            Input::File {
                path: Path::new("b.txt").into(),
                bytes: numbered(10).as_bytes().into(),
                language: None,
            },
        ));

        assert_eq!(
            frame.cursor().expect("a cursor").row,
            0,
            "the line was forgotten with the failed read"
        );
    }
}
