use std::iter::Peekable;

use crop::Rope;

use crate::view::{Selection, View};

/// Says what words are made of. Only letters and numbers make a word, `_` separates them just
/// like whitespace, so each part of a `snake_case_name` is its own word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::view) enum WordClass {
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
    /// Selects from the head through the word and the separators after it.
    pub(super) fn word_forward(&self) -> Option<Selection> {
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
    pub(super) fn word_backward(&mut self) -> Option<Selection> {
        let end = self.clamped_grapheme_end(self.selection.head());
        let rope = self.buffer.rope();
        let mut graphemes = BackwardGraphemes::new(rope, end, &mut self.word_scratch).peekable();

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

/// Walks the graphemes before a byte from the last to the first, as their start and class.
///
/// `crop` overflows walking a rope backward over some emoji sequences, such as a family
/// joined with zero-width joiners. So each line is walked forward into `line`, which this
/// borrows so that its memory is reused, and handed out from its end.
struct BackwardGraphemes<'a> {
    rope: &'a Rope,
    line: &'a mut Vec<(usize, WordClass)>,
    /// How many lines before the one in `line` are still to read.
    lines_left: usize,
    /// Where the text still to read ends.
    limit: usize,
}

impl<'a> BackwardGraphemes<'a> {
    fn new(rope: &'a Rope, end: usize, line: &'a mut Vec<(usize, WordClass)>) -> Self {
        line.clear();
        Self {
            rope,
            line,
            lines_left: rope.line_of_byte(end) + 1,
            limit: end,
        }
    }
}

impl Iterator for BackwardGraphemes<'_> {
    type Item = (usize, WordClass);

    fn next(&mut self) -> Option<Self::Item> {
        // A line can be empty, the one a cursor sits at the start of, or the one after a
        // final line break
        while self.line.is_empty() {
            self.lines_left = self.lines_left.checked_sub(1)?;
            let from = self.rope.byte_of_line(self.lines_left);
            let mut byte = from;
            self.line.extend(
                self.rope
                    .byte_slice(from..self.limit)
                    .graphemes()
                    .map(|grapheme| {
                        let start = byte;
                        byte += grapheme.len();
                        (start, word_class(&grapheme))
                    }),
            );
            self.limit = from;
        }
        self.line.pop()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.line.len(), None)
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
    use crate::view::{ColumnPolicy, Direction, Motion, tests};

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

    #[test]
    fn word_backward_walks_over_an_emoji_sequence() {
        // Only letters make a word, so the family is punctuation, one grapheme of it
        let family = "👨\u{200d}👩\u{200d}👧";
        let text = format!("ab {family}{family} cd");
        let mut view = tests::view(&text, 40, 5);
        view.set_selection(Selection::point(text.len() - 1), ColumnPolicy::Sync);
        view.move_selection(Motion::Word(Direction::Backward), false);
        assert_eq!(
            view.selection.head(),
            3 + 2 * family.len() + 1,
            "back to the start of cd"
        );
        view.move_selection(Motion::Word(Direction::Backward), false);
        assert_eq!(view.selection.head(), 3, "both families are one run");
    }

    #[test]
    fn word_backward_crosses_lines_and_empty_lines() {
        let mut view = tests::view("one\n\n\ntwo\n", 40, 3);
        view.set_selection(
            Selection::point(view.buffer.rope().byte_len()),
            ColumnPolicy::Sync,
        );
        view.move_selection(Motion::Word(Direction::Backward), false);
        assert_eq!(view.selection.head(), 6, "back to the start of two");
        view.move_selection(Motion::Word(Direction::Backward), false);
        assert_eq!(view.selection.head(), 0, "over the empty lines onto one");
    }
}
