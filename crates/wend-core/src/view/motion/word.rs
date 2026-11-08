use std::iter::Peekable;

use crate::view::{Selection, View};

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
    pub(super) fn word_backward(&self) -> Option<Selection> {
        let end = self.clamped_grapheme_end(self.selection.head());
        let mut graphemes = self
            .buffer
            .rope()
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
    use crate::view::{Direction, Motion, tests};

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
}
