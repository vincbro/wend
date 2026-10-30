use std::iter::Peekable;

use super::{ColumnPolicy, Selection, View, layout::SlotPosition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Backward,
    Forward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// Moves one grapheme left or right, following a wrapped line onto its next row.
    Horizontal(Direction),
    /// Moves one row up or down, aiming for the same column.
    Vertical(Direction),
    /// Selects the next or previous word.
    Word(Direction),
    /// Moves to the start or end of the line.
    Line(Direction),
    /// Moves a screen up or down.
    Page(Direction),
    /// Moves to the start or end of the file.
    File(Direction),
}

/// Says what words are made of. Only letters and numbers make a word, `_` separates them just
/// like whitespace, so each part of a `snake_case_name` is its own word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordClass {
    Word,
    Separator,
    Punctuation,
    /// Never part of a word selection, a word motion stops at it and skips it.
    LineBreak,
}

fn word_class(grapheme: &str) -> WordClass {
    match grapheme.chars().next() {
        Some(c) if c.is_alphanumeric() => WordClass::Word,
        Some('\n' | '\r') => WordClass::LineBreak,
        Some(c) if c.is_whitespace() || c == '_' => WordClass::Separator,
        _ => WordClass::Punctuation,
    }
}

impl View {
    /// Moves the selection. With `extend` the anchor stays and only the head moves.
    pub fn move_selection(&mut self, motion: Motion, extend: bool) {
        // Motions read the layout around the head, make sure both are current
        self.ensure_cursor_visible();
        let moved = match motion {
            Motion::Page(direction) => self.page(direction).map(Selection::point),
            _ => self.motion(motion),
        };
        let Some(moved) = moved else {
            return;
        };

        let selection = if extend {
            Selection::new(self.selection.anchor(), moved.head())
        } else {
            moved
        };
        let policy = match motion {
            Motion::Vertical(_) | Motion::Page(_) => ColumnPolicy::Keep,
            _ => ColumnPolicy::Sync,
        };
        self.set_selection(selection, policy);
        self.check_invariants();
    }

    /// Where a motion would put the selection, without moving it.
    fn motion(&self, motion: Motion) -> Option<Selection> {
        Some(match motion {
            Motion::Horizontal(direction) => Selection::point(self.horizontal(direction)?),
            Motion::Vertical(direction) => Selection::point(self.vertical(direction)?),
            Motion::Line(direction) => Selection::point(self.line_edge(direction)),
            Motion::File(direction) => Selection::point(self.file_edge(direction)),
            Motion::Word(Direction::Forward) => self.word_forward()?,
            Motion::Word(Direction::Backward) => self.word_backward()?,
            // Scrolls the view, so it can't be worked out without moving
            Motion::Page(_) => return None,
        })
    }

    /// Scrolls a page and returns where the head goes, none if it can't move.
    ///
    /// Lands where moving up or down a row at a time would, a screen less one row.
    fn page(&mut self, direction: Direction) -> Option<usize> {
        let row = match direction {
            Direction::Forward => self.page_forward()?,
            Direction::Backward => self.page_backward(),
        };
        let head = self.closest_glyph_start(row)?;
        // Moving a row at a time stops on the first row that can't move, and leaves the
        // selection as it was, a range included
        (head != self.selection.head()).then_some(head)
    }

    /// Scrolls down a page and returns the row the head lands on, none if no row shows text.
    fn page_forward(&mut self) -> Option<usize> {
        // The terminal can report 0 rows
        let bottom = self.rect.area.height().saturating_sub(1);
        let scrolled = self.scroll(Direction::Forward, bottom);
        let last = self.layout.last_row()?;
        if bottom - scrolled <= last {
            return Some(bottom - scrolled);
        }

        // The buffer ends first. Row by row the head would stop on its last row, with the
        // view scrolled no further than putting that row on the bottom margin.
        self.scroll(Direction::Backward, (bottom - last).min(scrolled));
        self.layout.last_row()
    }

    /// Scrolls up a page and returns the row the head lands on.
    fn page_backward(&mut self) -> usize {
        // The terminal can report 0 rows
        let bottom = self.rect.area.height().saturating_sub(1);
        let scrolled = self.scroll(Direction::Backward, bottom);
        // The top of the buffer stops the scroll short, the head stops on row 0 then
        scrolled
    }

    fn horizontal(&self, direction: Direction) -> Option<usize> {
        let pos = self.layout.find_byte(self.selection.head())?;
        if let Some(glyph) = self.layout.step(pos, direction) {
            return Some(glyph.from_byte);
        }

        // At the edge of the row, only continue if the line wrapped onto the next row
        let neighbour = match direction {
            Direction::Forward => pos.row + 1,
            Direction::Backward => pos.row.checked_sub(1)?,
        };
        if self.layout.row(neighbour)?.metadata.line != self.layout.row(pos.row)?.metadata.line {
            return None;
        }
        // Forward lands on the neighbour's first glyph, backward on its last
        self.layout
            .edge_glyph(neighbour, direction)
            .map(|glyph| glyph.from_byte)
    }

    fn vertical(&self, direction: Direction) -> Option<usize> {
        let pos = self.layout.find_byte(self.selection.head())?;
        let row = match direction {
            Direction::Forward => pos.row + 1,
            Direction::Backward => pos.row.checked_sub(1)?,
        };
        self.closest_glyph_start(row)
    }

    /// Returns the glyph on `row` at the column up and down aim for, or the closest one to it
    /// on a shorter row.
    fn closest_glyph_start(&self, row: usize) -> Option<usize> {
        let target = SlotPosition::new(self.desired_col, row);
        let glyph = self
            .layout
            .glyph_at(target)
            .or_else(|| self.layout.step(target, Direction::Backward))
            .or_else(|| self.layout.step(target, Direction::Forward))?;
        Some(glyph.from_byte)
    }

    /// Returns the first or last grapheme of the head's line, the line break not included.
    fn line_edge(&self, direction: Direction) -> usize {
        let rope = self.buffer.rope();
        let line = rope.line_of_byte(self.selection.head());
        let start = rope.byte_of_line(line);
        if direction == Direction::Backward {
            return start;
        }

        let end = if line < rope.line_len() {
            start + rope.line(line).byte_len()
        } else {
            rope.byte_len()
        };
        // An empty line, stay on its line break
        if end == start {
            return start;
        }
        self.grapheme_start(end - 1)
    }

    fn file_edge(&self, direction: Direction) -> usize {
        match direction {
            Direction::Backward => 0,
            Direction::Forward => {
                let len = self.buffer.rope().byte_len();
                self.grapheme_start(len.saturating_sub(1))
            }
        }
    }

    /// Selects from the head through the word and the separators after it.
    fn word_forward(&self) -> Option<Selection> {
        let head = self.selection.head();
        let mut graphemes = self
            .buffer
            .rope()
            .byte_slice(head..)
            .graphemes()
            .scan(head, |byte, grapheme| {
                let start = *byte;
                *byte += grapheme.len();
                Some((start, word_class(&grapheme)))
            })
            .peekable();

        let (anchor, class) = word_start(&mut graphemes)?;
        let head = take_run(&mut graphemes, class, anchor);
        let head = take_run(&mut graphemes, WordClass::Separator, head);
        Some(Selection::new(anchor, head))
    }

    /// Selects from the head back through any separators and the word before them.
    fn word_backward(&self) -> Option<Selection> {
        let rope = self.buffer.rope();
        let end = self
            .grapheme_end(self.selection.head())
            .min(rope.byte_len());
        let mut graphemes = rope
            .byte_slice(..end)
            .graphemes()
            .rev()
            .scan(end, |byte, grapheme| {
                *byte -= grapheme.len();
                Some((*byte, word_class(&grapheme)))
            })
            .peekable();

        let (anchor, class) = word_start(&mut graphemes)?;
        let mut head = take_run(&mut graphemes, class, anchor);
        // Separators alone aren't a word, take the word before them too, unless the
        // separators start the line
        if class == WordClass::Separator
            && let Some((byte, word)) = graphemes.next_if(|&(_, next)| next != WordClass::LineBreak)
        {
            head = take_run(&mut graphemes, word, byte);
        }
        Some(Selection::new(anchor, head))
    }
}

/// Returns where a word motion starts, as the byte and class of its first grapheme.
fn word_start<I>(graphemes: &mut Peekable<I>) -> Option<(usize, WordClass)>
where
    I: Iterator<Item = (usize, WordClass)>,
{
    let first = graphemes.next()?;
    // On the last grapheme of a run start on the next one, so repeating the motion walks
    // word by word instead of staying put
    let (mut byte, mut class) = graphemes
        .next_if(|&(_, next)| next != first.1)
        .unwrap_or(first);
    // A word never starts on a line break, move past it
    while class == WordClass::LineBreak {
        (byte, class) = graphemes.next()?;
    }
    Some((byte, class))
}

/// Takes graphemes while they are of `class`. Returns the byte of the last one taken,
/// `from` if none were.
fn take_run<I>(graphemes: &mut Peekable<I>, class: WordClass, from: usize) -> usize
where
    I: Iterator<Item = (usize, WordClass)>,
{
    let mut last = from;
    while let Some((byte, _)) = graphemes.next_if(|&(_, next)| next == class) {
        last = byte;
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::tests;

    #[test]
    fn word_stops_at_line_breaks() {
        // f0 o1 o2 ' '3 ' '4 '\n'5 b6 a7 r8
        let mut view = tests::view("foo  \nbar", 40, 5);
        view.move_selection(Motion::Word(Direction::Forward), false);
        assert_eq!(
            view.selection,
            Selection::new(0, 4),
            "word and spaces, not the newline"
        );
        view.move_selection(Motion::Word(Direction::Forward), false);
        assert_eq!(
            view.selection,
            Selection::new(6, 8),
            "skips the newline onto bar"
        );

        view.move_selection(Motion::Word(Direction::Backward), false);
        assert_eq!(
            view.selection,
            Selection::new(8, 6),
            "back to the start of bar"
        );
        view.move_selection(Motion::Word(Direction::Backward), false);
        assert_eq!(
            view.selection,
            Selection::new(4, 0),
            "over the newline onto foo"
        );
    }

    #[test]
    fn page_lands_where_moving_a_row_at_a_time_does() {
        assert_pages_like_rows(Direction::Forward);
        assert_pages_like_rows(Direction::Backward);
    }

    /// Pages one view and moves another a row at a time, from the far end of the buffer,
    /// checking both stay in the same place.
    fn assert_pages_like_rows(direction: Direction) {
        // Wrapped, short and empty lines, a page crosses all of them
        let text = "a line that wraps twice\nb\n\n\tc\n".repeat(20);
        let mut paged = tests::view(&text, 8, 7);
        let mut stepped = tests::view(&text, 8, 7);
        if direction == Direction::Backward {
            paged.move_selection(Motion::File(Direction::Forward), false);
            stepped.move_selection(Motion::File(Direction::Forward), false);
        }

        // Far enough to run into the other end of the buffer
        for page in 0..30 {
            paged.move_selection(Motion::Page(direction), false);
            // 7 rows high, a page is 6 rows
            for _ in 0..6 {
                stepped.move_selection(Motion::Vertical(direction), false);
            }
            assert_eq!(
                (paged.selection, paged.scroll_offset),
                (stepped.selection, stepped.scroll_offset),
                "page {page} {direction:?}"
            );
        }
    }

    #[test]
    fn page_that_cannot_move_keeps_the_selection() {
        let mut view = tests::view("foo bar\nbaz", 20, 5);
        view.move_selection(Motion::Word(Direction::Forward), false);
        let word = view.selection;
        view.move_selection(Motion::Page(Direction::Backward), false);
        assert_eq!(view.selection, word, "already on the top row");
    }

    #[test]
    fn word_treats_underscore_as_a_separator() {
        let mut view = tests::view("long_string_with_underscores", 40, 5);
        let mut anchors = vec![];
        for _ in 0..4 {
            view.move_selection(Motion::Word(Direction::Forward), false);
            anchors.push(view.selection.anchor());
        }
        // long, string, with, underscores
        assert_eq!(anchors, [0, 5, 12, 17], "one stop per part of the name");

        view.move_selection(Motion::Word(Direction::Backward), false);
        assert_eq!(
            view.selection.head(),
            17,
            "back to the start of underscores"
        );
        view.move_selection(Motion::Word(Direction::Backward), false);
        assert_eq!(view.selection.head(), 12, "back to the start of with");
    }
}
