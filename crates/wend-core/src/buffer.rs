use std::{
    ops,
    path::{Path, PathBuf},
    sync::Arc,
};

use crop::{Rope, RopeBuilder};

mod change;
pub use change::Change;
mod error;
pub use error::{Error, Result};

use crate::syntax::{Grammar, Highlight, LanguageTag, Syntax};

/// Holds the text of a file, plus its clean state.
///
/// There is no undo history. `clean_rope` is the text as it was last loaded or saved, and
/// undoing reverts to it. crop shares tree nodes between clones, so keeping it around only
/// costs the parts that differ.
#[derive(Debug, Clone)]
pub struct Buffer {
    path: Option<PathBuf>,
    rope: Rope,
    clean_rope: Rope,
    is_dirty: bool,
    /// The language the text is written in, whether or not its grammar is loaded yet.
    language: Option<LanguageTag>,
    /// None for plain text, and until the grammar of `language` is attached.
    syntax: Option<Syntax>,
}

impl Buffer {
    #[must_use]
    pub fn new() -> Self {
        Self::from_rope(None, Rope::new())
    }

    /// Sets the language the text is written in. A different language drops the syntax, which
    /// came from the old grammar, until the grammar of the new one is attached.
    pub fn set_language(&mut self, language: Option<LanguageTag>) {
        if language != self.language {
            self.syntax = None;
        }
        self.language = language;
    }

    /// Gives the buffer `grammar` to parse and highlight with, if it is the grammar of the
    /// buffer's language. Anything else, and the grammar the buffer already has, changes
    /// nothing.
    pub(crate) fn attach(&mut self, grammar: &Arc<Grammar>) {
        if self.language.as_ref() != Some(grammar.tag()) {
            return;
        }
        if self
            .syntax
            .as_ref()
            .is_some_and(|syntax| syntax.is_of(grammar))
        {
            return;
        }
        self.syntax = Some(Syntax::new(Arc::clone(grammar)));
    }

    #[must_use]
    pub(crate) fn syntax_mut(&mut self) -> Option<&mut Syntax> {
        self.syntax.as_mut()
    }

    /// The highlighted stretches of the text that touch `bytes`, see `Syntax::highlights`.
    /// There are none for plain text, or before the first parse.
    pub(crate) fn highlights(
        &mut self,
        bytes: ops::Range<usize>,
    ) -> impl Iterator<Item = (ops::Range<usize>, Highlight)> + '_ {
        let rope = &self.rope;
        self.syntax
            .as_mut()
            .map(|syntax| syntax.highlights(rope, bytes))
            .into_iter()
            .flatten()
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
            if let Some(syntax) = &mut self.syntax {
                syntax.reset();
            }
        }
        Ok(())
    }

    fn from_rope(path: Option<PathBuf>, rope: Rope) -> Self {
        Self {
            path,
            clean_rope: rope.clone(),
            rope,
            is_dirty: false,
            language: None,
            syntax: None,
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

        // Before the replace, the old end of the range only exists in the old text
        if let Some(syntax) = &mut self.syntax {
            syntax.edit(&self.rope, &range, text);
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
        // The edits being undone weren't recorded, so the tree can't follow them back
        if let Some(syntax) = &mut self.syntax {
            syntax.reset();
        }
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
    use crate::syntax;

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
    fn attach_takes_only_the_grammar_of_the_buffers_language() {
        let (_, grammar) = syntax::tests::rust();
        let mut buffer = Buffer::from("fn a() {}");

        buffer.attach(&grammar);
        assert!(
            buffer.syntax.is_none(),
            "a buffer with no language takes none"
        );

        buffer.set_language(Some(LanguageTag::new("python")));
        buffer.attach(&grammar);
        assert!(
            buffer.syntax.is_none(),
            "nor the grammar of another language"
        );

        buffer.set_language(Some(LanguageTag::new("rust")));
        buffer.attach(&grammar);
        assert!(buffer.syntax.is_some(), "its own language's grammar fits");
    }

    #[test]
    fn attaching_the_same_grammar_again_keeps_the_tree() {
        let text = "fn a() {}";
        let (loader, mut buffer) = syntax::tests::rust_buffer(text);
        let grammar = Arc::clone(loader.grammar(&LanguageTag::new("rust")).unwrap());

        buffer.attach(&grammar);

        assert_ne!(
            buffer.highlights(0..text.len()).count(),
            0,
            "the parsed tree is still there"
        );
    }

    #[test]
    fn a_newer_load_of_the_language_replaces_the_syntax() {
        let text = "fn a() {}";
        let (mut loader, mut buffer) = syntax::tests::rust_buffer(text);
        let newer = loader
            .load(
                LanguageTag::new("rust"),
                syntax::tests::RUST_WASM,
                syntax::tests::RUST_HIGHLIGHTS,
            )
            .unwrap();

        buffer.attach(&newer);

        assert_eq!(
            buffer.highlights(0..text.len()).count(),
            0,
            "the old tree came from the old grammar, the new one has to parse"
        );
    }

    #[test]
    fn changing_the_language_drops_the_syntax_and_keeping_it_does_not() {
        let text = "fn a() {}";
        let (_, mut buffer) = syntax::tests::rust_buffer(text);

        buffer.set_language(Some(LanguageTag::new("rust")));
        assert_ne!(
            buffer.highlights(0..text.len()).count(),
            0,
            "the same language keeps its tree"
        );

        buffer.set_language(Some(LanguageTag::new("python")));
        assert_eq!(
            buffer.highlights(0..text.len()).count(),
            0,
            "another language starts without one"
        );
    }

    #[test]
    fn a_revert_and_a_new_file_both_leave_the_tree_to_a_new_parse() {
        let text = "fn a() {}";
        let (mut loader, mut buffer) = syntax::tests::rust_buffer(text);

        buffer.edit(0..0, "x");
        buffer.revert();
        assert_eq!(
            buffer.highlights(0..text.len()).count(),
            0,
            "the undone edits weren't recorded, so the tree can't follow them"
        );
        loader.parse(&mut buffer).unwrap();
        assert_ne!(buffer.highlights(0..text.len()).count(), 0, "parsed again");

        buffer.update(b"fn b() {}").unwrap();
        assert_eq!(
            buffer.highlights(0..text.len()).count(),
            0,
            "the new text has no edit for the tree to follow either"
        );
    }
}
