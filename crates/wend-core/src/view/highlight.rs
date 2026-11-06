use std::{iter::Peekable, ops};

use crate::syntax::Highlight;

/// Finds the highlight of one byte after another, from the highlighted stretches that
/// `Syntax::highlights` gives.
pub(super) struct Highlighter<'a, I: Iterator<Item = (ops::Range<usize>, Highlight)>> {
    spans: Peekable<I>,
    /// The stretches the last byte is inside, with where each ends. The most specific one
    /// is last.
    open: &'a mut Vec<(usize, Highlight)>,
}

impl<'a, I: Iterator<Item = (ops::Range<usize>, Highlight)>> Highlighter<'a, I> {
    /// `open` is only borrowed so its memory outlives the highlighter and gets reused.
    pub(super) fn new(spans: I, open: &'a mut Vec<(usize, Highlight)>) -> Self {
        open.clear();
        Self {
            spans: spans.peekable(),
            open,
        }
    }

    /// The highlight of `byte`. The bytes asked for must never go down, because the
    /// stretches are used up as they are passed.
    pub(super) fn at(&mut self, byte: usize) -> Option<Highlight> {
        while let Some((span, highlight)) = self.spans.next_if(|(span, _)| span.start <= byte) {
            self.open.push((span.end, highlight));
        }
        self.open.retain(|(end, _)| *end > byte);
        self.open.last().map(|(_, highlight)| *highlight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_later_stretch_wins_where_they_overlap() {
        // A string from 2 to 10 with an escape inside it from 4 to 6
        let spans = [(2..10, Highlight::String), (4..6, Highlight::Escape)];
        let mut open = vec![];
        let mut highlighter = Highlighter::new(spans.into_iter(), &mut open);

        let found: Vec<_> = (0..11).map(|byte| highlighter.at(byte)).collect();

        let string = Some(Highlight::String);
        let escape = Some(Highlight::Escape);
        assert_eq!(
            found,
            [
                None, None, string, string, escape, escape, string, string, string, string, None
            ],
            "the string is 2..10 and the escape 4..6 wins inside it"
        );
    }
}
