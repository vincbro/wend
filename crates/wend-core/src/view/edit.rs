use std::ops;

use super::{ColumnPolicy, Selection, View};
use crate::buffer::{self, Change};

impl View {
    /// The only way the text changes. The selection follows the edit.
    fn edit(&mut self, range: ops::Range<usize>, text: &str) -> Change {
        let change = self.buffer.edit(range, text);
        self.layout_dirty = true;
        self.set_selection(self.selection.map(change), ColumnPolicy::Sync);
        change
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
        // Done on the buffer, not the layout, so it works across wrapped rows and lines
        let to_byte = self.selection.start();
        let Some(len) = self
            .buffer
            .rope()
            .byte_slice(..to_byte)
            .graphemes()
            .next_back()
            .map(|grapheme| grapheme.len())
        else {
            return;
        };
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

        self.edit(line_end..line_end, "\n");
        self.set_selection(Selection::point(line_end + 1), ColumnPolicy::Sync);
        self.check_invariants();
    }

    /// Throws away every edit since the file was last loaded or saved.
    pub fn revert(&mut self) {
        self.buffer.revert();
        self.layout_dirty = true;
        // There is no change to map through, keep the same bytes as close as the text allows
        self.set_selection(self.selection, ColumnPolicy::Sync);
        self.check_invariants();
    }

    pub fn save(&mut self) -> buffer::Result<()> {
        self.buffer.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::tests;

    #[test]
    fn delete_selection_collapses_onto_start() {
        let mut view = tests::view("hello world", 20, 5);
        view.set_selection(Selection::new(0, 4), ColumnPolicy::Sync);
        view.delete_selection();
        assert_eq!(view.buffer.rope().to_string(), " world");
        assert_eq!(view.selection, Selection::point(0));
    }

    #[test]
    fn delete_selection_handles_wide_graphemes() {
        let mut view = tests::view("a😀b", 20, 5);
        view.set_selection(Selection::point(1), ColumnPolicy::Sync);
        view.delete_selection();
        assert_eq!(view.buffer.rope().to_string(), "ab");
        assert_eq!(view.selection, Selection::point(1));
    }
}
