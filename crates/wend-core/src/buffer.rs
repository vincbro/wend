use std::{
    ops,
    path::{Path, PathBuf},
};

use crop::{Rope, RopeBuilder};
use tree_sitter::Tree;

mod change;
pub use change::Change;
mod error;
pub use error::{Error, Result};

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
    #[allow(
        dead_code,
        reason = "tree-sitter parsing is being built, nothing reads it yet"
    )]
    tree: Option<Tree>,
}

impl Buffer {
    #[must_use]
    pub fn new() -> Self {
        Self::from_rope(None, Rope::new())
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

        self.rope.replace(range, text);
        self.is_dirty = true;
        change
    }

    /// Throws away every edit since the last load or save.
    pub fn revert(&mut self) {
        // Copy, not swap, a swap would make the edits the new clean state
        self.rope = self.clean_rope.clone();
        self.is_dirty = false;
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
        assert!(!buffer.is_dirty());
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
}
