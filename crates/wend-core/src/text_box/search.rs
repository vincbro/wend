use std::ops::Range;

use crop::Rope;

use super::{ColumnPolicy, Direction, Selection, TextBox};

impl TextBox {
    /// Lets the box be searched: `/` asks its owner for a prompt, and the box keeps the query
    /// it is given for `n` and `p`. A box that isn't searchable ignores both.
    pub fn set_searchable(&mut self, searchable: bool) {
        self.searchable = searchable;
    }

    #[must_use]
    pub fn is_searchable(&self) -> bool {
        self.searchable
    }

    /// Makes `query` the one `Motion::Match` looks for, an empty one clears it.
    pub fn set_query(&mut self, query: &str) {
        self.query = Some(query)
            .filter(|query| !query.is_empty())
            .map(Into::into);
    }

    #[must_use]
    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// Puts the selection back to what it was, which the owner took with `selection`.
    pub fn restore_selection(&mut self, selection: Selection) {
        self.set_selection(selection, ColumnPolicy::Sync);
        self.check_invariants();
    }

    /// Selects the first match of `query` at or after the start of `origin`, wrapping around the
    /// end of the text. With none, or with an empty query, the selection goes back to `origin`.
    /// It is what a prompt does as the query is typed.
    pub fn find_from(&mut self, query: &str, origin: Selection) {
        let found = find_forward(
            self.buffer.rope(),
            &mut self.search_scratch,
            query,
            origin.start(),
        );
        match found {
            Some(range) => {
                let selection = self.match_selection(&range, Direction::Forward);
                self.show_match(selection);
            }
            None => self.set_selection(origin, ColumnPolicy::Sync),
        }
        self.check_invariants();
    }

    /// Puts the cursor at the start of `line`, counted from 1, or the last line when there are
    /// fewer. A line that is off screen is scrolled to the middle.
    pub fn go_to_line(&mut self, line: usize) {
        let rope = self.buffer.rope();
        let line = line
            .saturating_sub(1)
            .min(rope.line_len().saturating_sub(1));
        let byte = if rope.line_len() == 0 {
            0
        } else {
            rope.byte_of_line(line)
        };
        self.show_match(Selection::point(byte));
    }

    /// Where `Motion::Match` goes: the next match after the selection, or the previous one
    /// before it, selected whole. None without a query or a match.
    pub(super) fn match_motion(&mut self, direction: Direction) -> Option<Selection> {
        let query = self.query.as_deref()?;
        let rope = self.buffer.rope();
        let scratch = &mut self.search_scratch;
        let range = match direction {
            // After the last grapheme of the selection, so a match is never found again
            Direction::Forward => find_forward(rope, scratch, query, self.selection.end() + 1)?,
            Direction::Backward => find_backward(rope, scratch, query, self.selection.start())?,
        };
        Some(self.match_selection(&range, direction))
    }

    /// The selection of the whole match: the head is where the motion ends, so it is the last
    /// grapheme going forward and the first going backward.
    fn match_selection(&self, range: &Range<usize>, direction: Direction) -> Selection {
        let last = self.grapheme_start(range.end - 1);
        match direction {
            Direction::Forward => Selection::new(range.start, last),
            Direction::Backward => Selection::new(last, range.start),
        }
    }
}

/// The text of line `index`, borrowed from the rope when it is in one chunk, and otherwise copied
/// into `scratch`, which keeps its memory between calls.
fn line_text<'text>(rope: &'text Rope, index: usize, scratch: &'text mut String) -> &'text str {
    let mut chunks = rope.line(index).chunks();
    let Some(first) = chunks.next() else {
        return "";
    };
    let Some(second) = chunks.next() else {
        return first;
    };
    scratch.clear();
    scratch.push_str(first);
    scratch.push_str(second);
    for chunk in chunks {
        scratch.push_str(chunk);
    }
    scratch
}

/// A query with no capital ignores case, the way the ASCII letters do. Other letters match as
/// they are, so a byte offset in the lowercase text is one in the text.
fn folds(query: &str) -> bool {
    !query.chars().any(char::is_uppercase)
}

fn matches_at(haystack: &[u8], needle: &[u8], fold: bool) -> bool {
    if fold {
        haystack.eq_ignore_ascii_case(needle)
    } else {
        haystack == needle
    }
}

/// The first start of `needle` in `haystack` at or after `from`.
fn position(haystack: &[u8], needle: &[u8], fold: bool, from: usize) -> Option<usize> {
    let last = haystack.len().checked_sub(needle.len())?;
    (from..=last).find(|&at| matches_at(&haystack[at..at + needle.len()], needle, fold))
}

/// The last start of `needle` in `haystack` before `below`.
fn position_before(haystack: &[u8], needle: &[u8], fold: bool, below: usize) -> Option<usize> {
    let last = haystack.len().checked_sub(needle.len())?;
    let top = last.min(below.checked_sub(1)?);
    (0..=top)
        .rev()
        .find(|&at| matches_at(&haystack[at..at + needle.len()], needle, fold))
}

/// The first character boundary of `rope` at or after `byte`, which is the end of the text for a
/// byte past it. A caller that steps one byte past a grapheme can land inside a character.
fn boundary_at_or_after(rope: &Rope, byte: usize) -> usize {
    (byte..=rope.byte_len())
        .find(|&at| rope.is_char_boundary(at))
        .unwrap_or_else(|| rope.byte_len())
}

/// The first match of `query` that starts at or after `from`, or else the first one in the text.
fn find_forward(
    rope: &Rope,
    scratch: &mut String,
    query: &str,
    from: usize,
) -> Option<Range<usize>> {
    let count = rope.line_len();
    if query.is_empty() || count == 0 {
        return None;
    }
    let fold = folds(query);
    let from = boundary_at_or_after(rope, from);
    let first = rope.line_of_byte(from).min(count - 1);
    // The line the search starts on is looked at twice, after `from` and then from its start
    for step in 0..=count {
        let index = (first + step) % count;
        let base = rope.byte_of_line(index);
        let line = line_text(rope, index, scratch);
        let skip = if step == 0 {
            from.saturating_sub(base)
        } else {
            0
        };
        if let Some(at) = position(line.as_bytes(), query.as_bytes(), fold, skip) {
            return Some(base + at..base + at + query.len());
        }
    }
    None
}

/// The last match of `query` that starts before `before`, or else the last one in the text.
fn find_backward(
    rope: &Rope,
    scratch: &mut String,
    query: &str,
    before: usize,
) -> Option<Range<usize>> {
    let count = rope.line_len();
    if query.is_empty() || count == 0 {
        return None;
    }
    let fold = folds(query);
    let before = boundary_at_or_after(rope, before);
    let first = rope.line_of_byte(before).min(count - 1);
    for step in 0..=count {
        let index = (first + count - step % count) % count;
        let base = rope.byte_of_line(index);
        let line = line_text(rope, index, scratch);
        let below = if step == 0 {
            before.saturating_sub(base)
        } else {
            usize::MAX
        };
        if let Some(at) = position_before(line.as_bytes(), query.as_bytes(), fold, below) {
            return Some(base + at..base + at + query.len());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        input::{Mode, action::TextBoxAction},
        text_box::{Motion, tests::text_box},
    };

    fn forward(text: &str, query: &str, from: usize) -> Option<Range<usize>> {
        find_forward(&Rope::from(text), &mut String::new(), query, from)
    }

    fn backward(text: &str, query: &str, before: usize) -> Option<Range<usize>> {
        find_backward(&Rope::from(text), &mut String::new(), query, before)
    }

    #[test]
    fn forward_finds_the_first_match_at_or_after_and_wraps() {
        const TEXT: &str = "ab ab\nab";

        assert_eq!(forward(TEXT, "ab", 0), Some(0..2));
        assert_eq!(forward(TEXT, "ab", 1), Some(3..5));
        assert_eq!(forward(TEXT, "ab", 4), Some(6..8), "on the next line");
        assert_eq!(forward(TEXT, "ab", 7), Some(0..2), "wraps to the start");
    }

    #[test]
    fn backward_finds_the_last_match_before_and_wraps() {
        const TEXT: &str = "ab ab\nab";

        assert_eq!(backward(TEXT, "ab", 6), Some(3..5));
        assert_eq!(backward(TEXT, "ab", 3), Some(0..2));
        assert_eq!(backward(TEXT, "ab", 0), Some(6..8), "wraps to the end");
    }

    #[test]
    fn a_query_with_no_capital_ignores_case_and_one_with_a_capital_does_not() {
        const TEXT: &str = "Foo foo";

        assert_eq!(forward(TEXT, "foo", 1), Some(4..7));
        assert_eq!(
            forward(TEXT, "Foo", 1),
            Some(0..3),
            "only the capital one, wrapped"
        );
    }

    #[test]
    fn a_match_after_a_multibyte_character_has_the_right_offset() {
        const TEXT: &str = "漢字 字";

        assert_eq!(forward(TEXT, "字", 4), Some(7..10));
    }

    #[test]
    fn no_match_and_an_empty_query_find_nothing() {
        assert_eq!(forward("abc", "x", 0), None);
        assert_eq!(forward("abc", "", 0), None);
        assert_eq!(backward("", "a", 0), None);
    }

    #[test]
    fn a_text_that_spans_chunks_is_still_found() {
        let text = format!("{}needle\nx", "a".repeat(5000));

        assert_eq!(
            forward(&text, "needle", 0),
            Some(5000..5006),
            "a long line is in more than one chunk of the rope"
        );
    }

    #[test]
    fn in_select_mode_a_match_extends_the_selection() {
        let mut text_box = text_box("one two one", 20, 3);
        text_box.set_query("one");
        text_box.set_mode(Mode::Select);

        text_box.perform(TextBoxAction::Motion(Motion::Match(Direction::Forward)));

        assert_eq!(
            text_box.selection(),
            Selection::new(0, 10),
            "the anchor stays and the head goes to the end of the match"
        );
    }

    #[test]
    fn a_box_that_is_not_searchable_ignores_the_find_action() {
        let mut text_box = text_box("abc", 20, 5);

        text_box.perform(TextBoxAction::Find);

        assert!(
            !text_box.is_finding(),
            "only the owner makes a box searchable"
        );
    }

    #[test]
    fn n_selects_the_next_match_whole_and_p_the_previous_one() {
        let mut text_box = text_box("one two one", 20, 3);
        text_box.set_query("one");

        text_box.perform(TextBoxAction::Motion(Motion::Match(Direction::Forward)));
        assert_eq!(text_box.selection(), Selection::new(8, 10), "the later one");

        text_box.perform(TextBoxAction::Motion(Motion::Match(Direction::Backward)));
        assert_eq!(
            text_box.selection(),
            Selection::new(2, 0),
            "back to the first, head first"
        );
    }
}
