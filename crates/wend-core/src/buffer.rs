use std::{
    ops,
    path::{Path, PathBuf},
};

use crop::{Rope, RopeBuilder};
use tree_sitter::{InputEdit, Point, Tree};

mod change;
pub use change::Change;
mod error;
pub use error::{Error, Result};

use crate::syntax::{self, Highlight, LanguageTag, Syntax};

/// Holds the text of a file, plus its clean state.
///
/// There is no undo history. `clean` is the text as it was last loaded or saved, and
/// undoing reverts to it. crop shares tree nodes between clones, so keeping it around only
/// costs the parts that differ.
#[derive(Debug, Clone)]
pub struct Buffer {
    path: Option<PathBuf>,
    rope: Rope,
    clean_rope: Rope,
    is_dirty: bool,
    language: Option<LanguageTag>,
    tree: Option<Tree>,
    /// Set when the text or language changed since the last parse, so the tree doesn't
    /// describe the text yet.
    is_tree_stale: bool,
}

impl Buffer {
    #[must_use]
    pub fn new() -> Self {
        Self::from_rope(None, Rope::new())
    }

    /// Sets the language to parse the text as. A different language drops the tree, which
    /// came from the old grammar.
    pub fn set_language(&mut self, language: Option<LanguageTag>) {
        if language != self.language {
            self.tree = None;
            self.is_tree_stale = true;
        }
        self.language = language;
    }

    /// Brings the tree up to date with the text, and does nothing when it already is. Plain
    /// text has no tree. While the language's grammar isn't loaded the tree stays stale, so
    /// the next call tries again.
    pub fn reparse(&mut self, syntax: &mut Syntax) -> syntax::Result<()> {
        let Some(language) = &self.language else {
            return Ok(());
        };
        if !self.is_tree_stale {
            return Ok(());
        }
        let tree = syntax.parse(language, &self.rope, self.tree.as_ref())?;
        self.tree = Some(tree);
        self.is_tree_stale = false;
        Ok(())
    }

    /// The highlighted stretches of the text that touch `bytes`, see `Syntax::highlights`.
    /// There are none for plain text, or until the grammar is loaded and has parsed the text.
    pub fn highlights<'a>(
        &'a self,
        syntax: &'a mut Syntax,
        bytes: ops::Range<usize>,
    ) -> impl Iterator<Item = (ops::Range<usize>, Highlight)> + 'a {
        let spans = self
            .language
            .as_ref()
            .zip(self.tree.as_ref())
            .and_then(|(language, tree)| syntax.highlights(language, tree, &self.rope, bytes).ok());
        spans.into_iter().flatten()
    }

    pub fn from_bytes(path: PathBuf, bytes: &[u8]) -> Result<Self> {
        let content = str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)?;
        let mut rope_builder = RopeBuilder::new();
        rope_builder.append(content);
        Ok(Self::from_rope(Some(path), rope_builder.build()))
    }

    /// Takes in the file's new contents as the clean state. Without unsaved edits the text
    /// becomes them too, with unsaved edits it stays as it is.
    pub fn update(&mut self, bytes: &[u8]) -> Result<()> {
        let content = str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)?;
        let mut rope_builder = RopeBuilder::new();
        rope_builder.append(content);
        self.clean_rope = rope_builder.build();
        if !self.is_dirty {
            self.rope = self.clean_rope.clone();
            // The whole text is new, there is no edit for the tree to follow
            self.tree = None;
            self.is_tree_stale = true;
        }
        Ok(())
    }

    fn from_rope(path: Option<PathBuf>, rope: Rope) -> Self {
        Self {
            path,
            clean_rope: rope.clone(),
            rope,
            is_dirty: false,
            tree: None,
            language: None,
            is_tree_stale: true,
        }
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    #[must_use]
    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// Returns whether the text differs from the last load or save.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    /// Replaces `range` with `text`, the only way the text changes.
    ///
    /// # Panics
    ///
    /// If `range` is out of bounds or doesn't fall on char boundaries.
    pub fn edit(&mut self, range: ops::Range<usize>, text: &str) -> Change {
        assert!(
            range.start <= range.end && range.end <= self.rope.byte_len(),
            "edit ranges come from selections, which stay inside the rope, got {range:?}"
        );
        assert!(
            self.rope.is_char_boundary(range.start) && self.rope.is_char_boundary(range.end),
            "edit ranges come from selections, which sit on grapheme starts, got {range:?}"
        );

        let change = Change::new(range.start, range.len(), text.len());
        if range.is_empty() && text.is_empty() {
            return change;
        }

        // Measured before the replace, the old end only exists in the old text
        if let Some(tree) = &mut self.tree {
            tree.edit(&input_edit(&self.rope, &range, text));
        }
        self.rope.replace(range, text);
        self.is_dirty = true;
        self.is_tree_stale = true;
        change
    }

    /// Throws away every edit since the last load or save.
    pub fn revert(&mut self) {
        // Copy, not swap, a swap would make the edits the new clean state
        self.rope = self.clean_rope.clone();
        self.is_dirty = false;
        // The edits being undone weren't recorded, so the tree can't follow them back
        self.tree = None;
        self.is_tree_stale = true;
    }

    /// Makes the text the new clean state, and returns the path and text to write.
    pub fn save(&mut self) -> Result<(PathBuf, Rope)> {
        let Some(path) = &self.path else {
            return Err(Error::NoPath);
        };

        self.clean_rope = self.rope.clone();
        self.is_dirty = false;
        Ok((path.clone(), self.rope.clone()))
    }
}

/// Describes replacing `range` of `rope` with `text` the way tree-sitter needs it, in rows
/// and columns as well as bytes. Takes the rope from before the replace.
fn input_edit(rope: &Rope, range: &ops::Range<usize>, text: &str) -> InputEdit {
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

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl From<&str> for Buffer {
    fn from(text: &str) -> Self {
        Self::from_rope(None, Rope::from(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_applies_and_dirties() {
        let mut buffer = Buffer::from("hello world");
        let change = buffer.edit(6..11, "wend");
        assert_eq!(buffer.rope().to_string(), "hello wend", "world is replaced");
        assert_eq!(
            change,
            Change::new(6, 5, 4),
            "the change holds the removed and inserted lengths"
        );
        assert!(buffer.is_dirty(), "an edit dirties the buffer");
    }

    #[test]
    fn empty_edit_stays_clean() {
        let mut buffer = Buffer::from("abc");
        buffer.edit(1..1, "");
        assert!(!buffer.is_dirty(), "nothing was inserted or removed");
    }

    #[test]
    #[should_panic(expected = "grapheme starts")]
    fn edit_inside_a_char_panics() {
        let mut buffer = Buffer::from("é");
        buffer.edit(1..1, "x");
    }

    #[test]
    fn revert_restores_clean() {
        let mut buffer = Buffer::from("abc");
        buffer.edit(0..1, "X");
        buffer.edit(3..3, "Y");
        buffer.revert();
        assert_eq!(buffer.rope().to_string(), "abc");
        assert!(!buffer.is_dirty());
    }

    #[test]
    fn non_utf8_file_is_an_error() {
        let result = Buffer::from_bytes(PathBuf::from("latin1.txt"), b"caf\xe9");
        assert!(
            matches!(result, Err(Error::InvalidUtf8)),
            "a latin-1 file is refused, not a panic"
        );
    }

    #[test]
    fn save_without_a_path_is_an_error() {
        let mut buffer = Buffer::from("abc");
        assert!(matches!(buffer.save(), Err(Error::NoPath)));
    }

    #[test]
    fn save_returns_the_text_to_write() {
        let mut buffer = Buffer::from_bytes(PathBuf::from("a.txt"), b"abc").unwrap();
        buffer.edit(3..3, "d");
        let (path, rope) = buffer.save().unwrap();

        assert_eq!(path, PathBuf::from("a.txt"));
        assert_eq!(rope.to_string(), "abcd");
        assert!(!buffer.is_dirty(), "the saved text is the clean state");
    }

    #[test]
    fn save_then_revert_keeps_saved_text() {
        let mut buffer = Buffer::from_bytes(PathBuf::from("a.txt"), b"abc").unwrap();
        buffer.edit(3..3, "d");
        buffer.save().unwrap();
        buffer.edit(0..1, "X");
        buffer.revert();

        assert_eq!(buffer.rope().to_string(), "abcd");
    }

    #[test]
    fn update_without_edits_takes_the_new_text() {
        let mut buffer = Buffer::from_bytes(PathBuf::from("a.txt"), b"abc").unwrap();
        buffer.update(b"xyz").unwrap();
        assert_eq!(buffer.rope().to_string(), "xyz");
        assert!(!buffer.is_dirty());
    }

    #[test]
    fn update_with_edits_keeps_them() {
        let mut buffer = Buffer::from_bytes(PathBuf::from("a.txt"), b"abc").unwrap();
        buffer.edit(3..3, "d");
        buffer.update(b"xyz").unwrap();
        assert_eq!(
            buffer.rope().to_string(),
            "abcd",
            "unsaved edits aren't thrown away"
        );

        buffer.revert();
        assert_eq!(
            buffer.rope().to_string(),
            "xyz",
            "the new contents are the clean state"
        );
    }

    #[test]
    fn update_with_invalid_utf8_is_an_error() {
        let mut buffer = Buffer::from_bytes(PathBuf::from("a.txt"), b"abc").unwrap();
        assert!(matches!(buffer.update(b"caf\xe9"), Err(Error::InvalidUtf8)));
        assert_eq!(buffer.rope().to_string(), "abc", "the text is left alone");
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
