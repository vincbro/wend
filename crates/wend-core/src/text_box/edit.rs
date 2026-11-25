use std::ops;

use crop::Rope;

use super::{ColumnPolicy, Selection, TextBox, limits::visible_lines};

impl TextBox {
    /// The only way the user changes the text, which a read-only box refuses. The selection
    /// follows the edit.
    fn edit(&mut self, range: ops::Range<usize>, text: &str) {
        if self.is_read_only() {
            return;
        }
        let text = self.within_line_limit(&range, text);
        let change = self.buffer.edit(range, text);
        self.viewport.mark_dirty();
        self.set_selection(self.selection.map(change), ColumnPolicy::Sync);
    }

    /// Inserts in front of the selection.
    pub fn insert(&mut self, key: char) {
        let mut buf = [0; 4];
        let text = key.encode_utf8(&mut buf);
        let at = self.selection.start();
        self.edit(at..at, text);
        self.check_invariants();
    }

    /// Deletes the grapheme before the selection, joins lines when at the start of one.
    pub fn delete(&mut self) {
        let to_byte = self.selection.start();
        if to_byte == 0 {
            return;
        }
        // Asked for from the byte before, as `crop` can't walk back over every emoji sequence
        let len = to_byte - self.grapheme_start(to_byte - 1);
        self.edit(to_byte - len..to_byte, "");
        self.check_invariants();
    }

    /// Deletes everything the selection covers and collapses onto what follows.
    pub fn delete_selection(&mut self) {
        let range = self.selection_range();
        if range.is_empty() {
            return;
        }
        // Both ends are inside the deleted range, so they map onto its start
        self.edit(range, "");
        self.check_invariants();
    }

    /// Opens a new line below the head's line and puts the cursor on it.
    pub fn new_line(&mut self) {
        let line = self.buffer.rope().line_of_byte(self.selection.head());
        let line_end = self.line_end(line);

        let before = self.buffer.rope().byte_len();
        self.edit(line_end..line_end, "\n");
        if self.buffer.rope().byte_len() == before {
            // The line limit refused the break, so there is no line to go to
            self.check_invariants();
            return;
        }
        self.set_selection(Selection::point(line_end + 1), ColumnPolicy::Sync);
        self.check_invariants();
    }

    /// Returns the start of `text` that fits the line limit once it replaces `range`: all of it
    /// without a limit, otherwise up to the first line break that would go over.
    fn within_line_limit<'a>(&self, range: &ops::Range<usize>, text: &'a str) -> &'a str {
        let Some(limit) = self.limits.line_limit() else {
            return text;
        };
        let rope = self.buffer.rope();
        let removed = rope.line_of_byte(range.end) - rope.line_of_byte(range.start);
        let kept = visible_lines(rope).saturating_sub(removed).max(1);
        let room = limit.get().saturating_sub(kept);
        let Some((at, _)) = text.match_indices('\n').nth(room) else {
            return text;
        };
        // A CRLF is one break, so the carriage return goes with it
        let text = &text[..at];
        text.strip_suffix('\r').unwrap_or(text)
    }

    /// Cuts off the text past the line limit. The buffer's clean text is left as it is, so this
    /// is what a revert or a new file has to do again.
    pub(super) fn clip_to_line_limit(&mut self) {
        let Some(limit) = self.limits.line_limit() else {
            return;
        };
        if visible_lines(self.buffer.rope()) <= limit.get() {
            return;
        }
        let from = self.line_end(limit.get() - 1);
        let to = self.buffer.rope().byte_len();
        self.buffer.edit(from..to, "");
    }

    /// Throws away every edit since the file was last loaded or saved.
    pub fn revert(&mut self) {
        if self.is_read_only() {
            return;
        }
        self.buffer.revert();
        self.clip_to_line_limit();
        self.viewport.mark_dirty();
        // There is no change to map through, keep the same bytes as close as the text allows
        self.set_selection(self.selection, ColumnPolicy::Sync);
        self.check_invariants();
    }

    /// The text, without making it the clean state.
    #[must_use]
    pub fn text(&self) -> Rope {
        self.buffer.rope().clone()
    }

    /// Makes the text the new clean state, and returns it to write.
    pub fn save(&mut self) -> Rope {
        self.buffer.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_box::tests;

    #[test]
    fn delete_selection_collapses_onto_start() {
        let mut view = tests::text_box("hello world", 20, 5);
        view.set_selection(Selection::new(0, 4), ColumnPolicy::Sync);
        view.delete_selection();
        assert_eq!(
            view.buffer.rope().to_string(),
            " world",
            "the selected hello is gone"
        );
        assert_eq!(
            view.selection,
            Selection::point(0),
            "the cursor takes its place"
        );
    }

    #[test]
    fn delete_selection_handles_wide_graphemes() {
        let mut view = tests::text_box("a😀b", 20, 5);
        view.set_selection(Selection::point(1), ColumnPolicy::Sync);
        view.delete_selection();
        assert_eq!(
            view.buffer.rope().to_string(),
            "ab",
            "the whole emoji is deleted, not half of it"
        );
        assert_eq!(view.selection, Selection::point(1), "the cursor stays on b");
    }

    #[test]
    fn delete_removes_a_whole_emoji_sequence() {
        // The family is three emoji joined by two zero-width joiners, one grapheme
        let family = "👨\u{200d}👩\u{200d}👧";
        let mut view = tests::text_box(&format!("a{family}b"), 20, 5);
        view.set_selection(Selection::point(1 + family.len()), ColumnPolicy::Sync);
        view.delete();
        assert_eq!(
            view.buffer.rope().to_string(),
            "ab",
            "all of the family goes"
        );
        assert_eq!(view.selection, Selection::point(1), "the cursor stays on b");
    }

    #[test]
    fn delete_removes_an_emoji_sequence_that_is_off_screen() {
        let family = "👨\u{200d}👩\u{200d}👧";
        let text = format!("{family}\n\n\n\n\n\n\nx");
        let mut view = tests::text_box(&text, 20, 2);
        // The head scrolls the text box down, which leaves the anchor behind the family off screen
        let selection = Selection::new(family.len(), text.len() - 1);
        view.set_selection(selection, ColumnPolicy::Sync);
        assert!(
            view.viewport.wrap().find_byte(family.len()).is_none(),
            "the anchor is off screen"
        );
        view.delete();
        assert_eq!(
            view.buffer.rope().to_string(),
            "\n\n\n\n\n\n\nx",
            "the whole family is gone"
        );
    }
}
