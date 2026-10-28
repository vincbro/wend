use wend_core::{
    area::Area,
    buffer::Buffer,
    editor::{Editor, Mode},
    frame::{CellPosition, Frame},
    view::{Direction, Motion},
};

fn editor(text: &str, cols: u16, rows: u16) -> Editor {
    let mut editor = Editor::new(Area::new(cols, rows));
    editor.view_mut().open_buffer(Buffer::from(text));
    editor
}

fn row_text(frame: &Frame, row: u16) -> String {
    let text: String = frame
        .row(row)
        .iter()
        .filter(|cell| !cell.covered)
        .map(|cell| cell.grapheme.as_str_sanitize())
        .collect();
    text.trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_inserts_in_front_of_the_cursor() {
        let mut editor = editor("world", 20, 3);
        editor.switch_mode(Mode::Insert);
        for c in "hello ".chars() {
            editor.view_mut().insert(c);
        }

        let frame = editor.frame();
        assert_eq!(row_text(&frame, 0), "hello world");
        assert_eq!(
            frame.cursor(),
            CellPosition::new(6, 0),
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
            CellPosition::new(1, 1),
            "on the h of three"
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
        assert_eq!(frame.cursor(), CellPosition::new(0, 0));
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
        let cursor = frame.cursor();
        assert!(cursor.row < 5, "the cursor is inside the frame");
        assert_eq!(
            row_text(&frame, cursor.row),
            "49",
            "the cursor's row shows the last line"
        );
        assert_eq!(cursor.col, 1, "on the last grapheme, the 9");
    }
}
