use std::ops::Range;

use crop::Rope;
use tree_sitter::{InputEdit, Point};

/// The text from `byte` to the end of the chunk it's in, which is what tree-sitter asks
/// for. Empty from the end of the rope on.
pub(super) fn text_from(rope: &Rope, byte: usize) -> &[u8] {
    if byte >= rope.byte_len() {
        return &[];
    }
    // tree-sitter asks for token starts, which are char boundaries, but a byte inside a
    // char must not panic: slice from the char's start and skip into it
    let mut start = byte;
    while !rope.is_char_boundary(start) {
        start -= 1;
    }
    let chunk = rope
        .byte_slice(start..)
        .chunks()
        .next()
        .expect("a slice that starts before the end of the rope holds at least one chunk");
    &chunk.as_bytes()[byte - start..]
}

/// Describes replacing `range` of `rope` with `text` the way tree-sitter needs it, in rows
/// and columns as well as bytes. Takes the rope from before the replace.
pub(super) fn input_edit(rope: &Rope, range: &Range<usize>, text: &str) -> InputEdit {
    let start_position = point(rope, range.start);
    InputEdit {
        start_byte: range.start,
        old_end_byte: range.end,
        new_end_byte: range.start + text.len(),
        start_position,
        old_end_position: point(rope, range.end),
        new_end_position: end_point(start_position, text),
    }
}

/// The row and column of `byte` the way tree-sitter counts them: rows end at `\n`, and the
/// column is in bytes from the start of the row.
fn point(rope: &Rope, byte: usize) -> Point {
    let row = rope.line_of_byte(byte);
    Point::new(row, byte - rope.byte_of_line(row))
}

/// Where `text` ends once it's inserted at `start`.
fn end_point(start: Point, text: &str) -> Point {
    match text.rfind('\n') {
        None => Point::new(start.row, start.column + text.len()),
        Some(last_break) => Point::new(
            start.row + text.matches('\n').count(),
            text.len() - last_break - 1,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads the rope from `byte` the way tree-sitter does, one piece after another.
    fn read_from(rope: &Rope, byte: usize) -> Vec<u8> {
        let mut text = vec![];
        loop {
            let piece = text_from(rope, byte + text.len());
            if piece.is_empty() {
                return text;
            }
            text.extend_from_slice(piece);
        }
    }

    #[test]
    fn reading_on_gives_the_whole_text() {
        // Long enough for crop to split it over many chunks
        let text = "fn é() {}\n".repeat(2_000);
        let rope = Rope::from(text.as_str());
        for byte in [0, 1, 4, 5, 1_000, text.len() - 1] {
            assert_eq!(
                read_from(&rope, byte),
                &text.as_bytes()[byte..],
                "from byte {byte}"
            );
        }
    }

    #[test]
    fn a_byte_inside_a_char_starts_inside_it() {
        // é is bytes 1..3
        let rope = Rope::from("aéb");
        assert_eq!(
            read_from(&rope, 2),
            b"\xa9b",
            "no panic, the rest of é and then b"
        );
    }

    #[test]
    fn the_end_is_empty() {
        let rope = Rope::from("ab");
        assert_eq!(text_from(&rope, 2), b"", "at the end");
        assert_eq!(text_from(&rope, 10), b"", "past the end too");
        assert_eq!(text_from(&Rope::new(), 0), b"", "an empty rope");
    }

    #[test]
    fn input_edit_inside_a_line() {
        // "ab|cd\nef" insert "XY" at 2
        let rope = Rope::from("abcd\nef");
        let edit = input_edit(&rope, &(2..2), "XY");
        assert_eq!(
            (edit.start_byte, edit.old_end_byte, edit.new_end_byte),
            (2, 2, 4)
        );
        assert_eq!(edit.start_position, Point::new(0, 2));
        assert_eq!(edit.old_end_position, Point::new(0, 2));
        assert_eq!(edit.new_end_position, Point::new(0, 4));
    }

    #[test]
    fn input_edit_adding_lines() {
        // "ab|cd" insert "X\nYZ\nW" at 2: the end moves two rows down, one byte in
        let rope = Rope::from("abcd");
        let edit = input_edit(&rope, &(2..2), "X\nYZ\nW");
        assert_eq!(edit.new_end_position, Point::new(2, 1));
    }

    #[test]
    fn input_edit_removing_lines() {
        // "a[b\ncd\ne]f" delete b..e, the old end is on the third row
        let rope = Rope::from("ab\ncd\nef");
        let edit = input_edit(&rope, &(1..7), "");
        assert_eq!(edit.start_position, Point::new(0, 1));
        assert_eq!(
            edit.old_end_position,
            Point::new(2, 1),
            "measured in the old text"
        );
        assert_eq!(edit.new_end_position, Point::new(0, 1));
    }

    #[test]
    fn input_edit_at_the_end_after_a_line_break() {
        let rope = Rope::from("ab\n");
        let edit = input_edit(&rope, &(3..3), "c");
        assert_eq!(
            edit.start_position,
            Point::new(1, 0),
            "after the last break is the start of an empty row"
        );
        assert_eq!(edit.new_end_position, Point::new(1, 1));
    }

    #[test]
    fn columns_count_bytes() {
        // é is two bytes
        let rope = Rope::from("éa");
        assert_eq!(
            point(&rope, 2),
            Point::new(0, 2),
            "tree-sitter columns are bytes, not chars"
        );
    }
}
