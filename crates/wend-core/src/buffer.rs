use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    ops,
    path::{Path, PathBuf},
};

use crop::{Rope, RopeBuilder};

use crate::grapheme::Grapheme;

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
    clean: Rope,
    dirty: bool,
}

impl Buffer {
    #[must_use]
    pub fn new() -> Self {
        Self::from_rope(None, Rope::new())
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        let content = fs::read(path)?;
        let content = str::from_utf8(&content).map_err(|_| Error::InvalidUtf8)?;

        let mut rope_builder = RopeBuilder::new();
        rope_builder.append(content);

        Ok(Self::from_rope(
            Some(path.to_path_buf()),
            rope_builder.build(),
        ))
    }

    fn from_rope(path: Option<PathBuf>, rope: Rope) -> Self {
        Self {
            path,
            clean: rope.clone(),
            rope,
            dirty: false,
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
        self.dirty
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
        self.dirty = true;
        change
    }

    /// Throws away every edit since the last load or save.
    pub fn revert(&mut self) {
        // Copy, not swap, a swap would make the edits the new clean state
        self.rope = self.clean.clone();
        self.dirty = false;
    }

    /// Writes the text to `path` and makes it the new clean state.
    pub fn save(&mut self) -> Result<()> {
        let Some(path) = &self.path else {
            return Err(Error::NoPath);
        };

        let mut file = BufWriter::new(File::create(path)?);
        for chunk in self.rope.chunks() {
            file.write_all(chunk.as_bytes())?;
        }
        file.flush()?;

        self.clean = self.rope.clone();
        self.dirty = false;
        Ok(())
    }

    /// # Panics
    ///
    /// If `range` is out of bounds or doesn't fall on char boundaries.
    #[must_use]
    pub fn grapheme(&self, range: ops::Range<usize>) -> Grapheme {
        Grapheme::from_bytes_iter(self.rope.byte_slice(range).bytes())
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
    use std::{env, process};

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
        let path = env::temp_dir().join(format!("wend-latin1-{}.txt", process::id()));
        fs::write(&path, b"caf\xe9").unwrap();

        let result = Buffer::from_path(&path);
        fs::remove_file(&path).unwrap();
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
    fn save_then_revert_keeps_saved_text() {
        let path = env::temp_dir().join(format!("wend-save-{}.txt", process::id()));
        fs::write(&path, "abc").unwrap();

        let mut buffer = Buffer::from_path(&path).unwrap();
        buffer.edit(3..3, "d");
        buffer.save().unwrap();
        buffer.edit(0..1, "X");
        buffer.revert();

        assert_eq!(buffer.rope().to_string(), "abcd");
        assert_eq!(fs::read_to_string(&path).unwrap(), "abcd");
        fs::remove_file(&path).unwrap();
    }
}
