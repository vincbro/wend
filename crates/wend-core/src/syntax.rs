use std::{fmt, ops::Range, sync::Arc};

use crop::Rope;
use tree_sitter::{
    InputEdit, Language, Node, Parser, Point, Query, QueryCursor, QueryMatch, StreamingIterator,
    Tree,
};

mod error;
pub use error::{Error, Result};
mod highlight;
pub use highlight::Highlight;
mod loader;
pub use loader::Loader;

/// Names a language, like `rust`. It is what a buffer is tagged with and what the frontend is
/// asked to provide the grammar for.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LanguageTag(Arc<str>);

impl LanguageTag {
    #[must_use]
    pub fn new(tag: &str) -> Self {
        Self(tag.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A language's parser and highlights query, loaded once by the `Loader` and shared by every
/// buffer written in it.
pub struct Grammar {
    tag: LanguageTag,
    language: Language,
    query: Query,
    /// What each of the query's captures highlights as, by capture index.
    captures: Box<[Option<Highlight>]>,
}

impl Grammar {
    #[must_use]
    pub fn tag(&self) -> &LanguageTag {
        &self.tag
    }
}

/// What a buffer knows about its text as code: the grammar it's written in, and the tree of
/// its last parse.
///
/// An edit moves the tree along and marks it stale, `Loader::parse` brings it up to date.
/// Highlighting works from whatever tree there is, so it never waits for a parse.
pub(crate) struct Syntax {
    grammar: Arc<Grammar>,
    tree: Option<Tree>,
    /// Set when the text changed since the last parse, so the tree doesn't describe it yet.
    is_stale: bool,
    cursor: QueryCursor,
}

impl Syntax {
    #[must_use]
    pub(crate) fn new(grammar: Arc<Grammar>) -> Self {
        Self {
            grammar,
            tree: None,
            is_stale: true,
            cursor: QueryCursor::new(),
        }
    }

    /// Returns whether this is the syntax of `grammar` itself, and not of an older load of
    /// the same language.
    #[must_use]
    pub(crate) fn is_of(&self, grammar: &Arc<Grammar>) -> bool {
        Arc::ptr_eq(&self.grammar, grammar)
    }

    /// Moves the tree along with replacing `range` of `rope` with `text`. Takes the rope
    /// from before the replace, where the old end of the range still exists.
    pub(crate) fn edit(&mut self, rope: &Rope, range: &Range<usize>, text: &str) {
        if let Some(tree) = &mut self.tree {
            tree.edit(&input_edit(rope, range, text));
        }
        self.is_stale = true;
    }

    /// Throws the tree away, for text that was replaced as a whole and has no edit to follow.
    pub(crate) fn reset(&mut self) {
        self.tree = None;
        self.is_stale = true;
    }

    /// Parses `rope` if the tree is stale, reading it chunk by chunk so the text is never
    /// copied. The parse reuses every part of the old tree the edits didn't touch.
    fn parse(&mut self, parser: &mut Parser, rope: &Rope) -> Result<()> {
        if !self.is_stale {
            return Ok(());
        }
        parser.set_language(&self.grammar.language)?;
        let tree = parser
            .parse_with_options(
                &mut |byte, _| text_from(rope, byte),
                self.tree.as_ref(),
                None,
            )
            .ok_or_else(|| Error::ParseStopped(self.grammar.tag.clone()))?;
        self.tree = Some(tree);
        self.is_stale = false;
        Ok(())
    }

    /// The highlighted stretches of `rope` that touch `bytes`, in the order they start. Nothing
    /// is highlighted until the first parse.
    ///
    /// Stretches nest and overlap, an escape inside a string for one, and the one that
    /// comes later is the more specific. The first ones can start before `bytes`.
    pub(crate) fn highlights<'a>(
        &'a mut self,
        rope: &'a Rope,
        bytes: Range<usize>,
    ) -> impl Iterator<Item = (Range<usize>, Highlight)> + 'a {
        let Self {
            grammar,
            tree,
            cursor,
            ..
        } = self;
        // Shared, so that both closures below can hold it
        let grammar: &Grammar = grammar;
        cursor.set_byte_range(bytes);
        let text = |node: Node<'_>| {
            rope.byte_slice(node.byte_range())
                .chunks()
                .map(str::as_bytes)
        };
        // Captures without a highlight, like the ones a query only uses in predicates, are
        // skipped
        tree.as_ref()
            .map(move |tree| {
                cursor
                    .captures(&grammar.query, tree.root_node(), text)
                    .filter_map(move |(query_match, index)| {
                        capture_highlight(grammar, query_match, *index)
                    })
                    .cloned()
            })
            .into_iter()
            .flatten()
    }
}

impl Clone for Syntax {
    /// The cursor is scratch space, so the clone gets a new one.
    fn clone(&self) -> Self {
        Self {
            grammar: Arc::clone(&self.grammar),
            tree: self.tree.clone(),
            is_stale: self.is_stale,
            cursor: QueryCursor::new(),
        }
    }
}

impl fmt::Debug for Syntax {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Syntax")
            .field("language", self.grammar.tag())
            .field("is_stale", &self.is_stale)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for Grammar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Grammar")
            .field("tag", &self.tag)
            .finish_non_exhaustive()
    }
}

/// The stretch of text and highlight of capture `index` of `query_match`, none for a capture
/// without a highlight.
fn capture_highlight(
    grammar: &Grammar,
    query_match: &QueryMatch<'_, '_>,
    index: usize,
) -> Option<(Range<usize>, Highlight)> {
    let capture = query_match.captures()[index];
    let highlight = grammar.captures[capture.index as usize]?;
    Some((capture.node.byte_range(), highlight))
}

/// The text from `byte` to the end of the chunk it's in, which is what tree-sitter asks
/// for. Empty from the end of the rope on.
fn text_from(rope: &Rope, byte: usize) -> &[u8] {
    if byte >= rope.byte_len() {
        return &[];
    }
    // tree-sitter asks for token starts, which are char boundaries, but a byte inside a
    // char must not panic: slice from the char's start and skip into it
    let mut start = byte;
    while !rope.is_char_boundary(start) {
        start -= 1;
    }
    let chunk = rope
        .byte_slice(start..)
        .chunks()
        .next()
        .expect("a slice that starts before the end of the rope holds at least one chunk");
    &chunk.as_bytes()[byte - start..]
}

/// Describes replacing `range` of `rope` with `text` the way tree-sitter needs it, in rows
/// and columns as well as bytes. Takes the rope from before the replace.
fn input_edit(rope: &Rope, range: &Range<usize>, text: &str) -> InputEdit {
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

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::buffer::Buffer;

    pub(crate) const RUST_WASM: &[u8] = include_bytes!("../../../tree-sitter-rust.wasm");
    pub(crate) const RUST_HIGHLIGHTS: &[u8] = include_bytes!("../../../highlights.scm");

    /// A loader that has the Rust grammar, and that grammar.
    pub(crate) fn rust() -> (Loader, Arc<Grammar>) {
        let mut loader = Loader::new().unwrap();
        let grammar = loader
            .load(LanguageTag::new("rust"), RUST_WASM, RUST_HIGHLIGHTS)
            .unwrap();
        (loader, grammar)
    }

    /// A buffer of Rust code, parsed.
    pub(crate) fn rust_buffer(text: &str) -> (Loader, Buffer) {
        let (mut loader, grammar) = rust();
        let mut buffer = Buffer::from(text);
        buffer.set_language(Some(LanguageTag::new("rust")));
        buffer.attach(&grammar);
        loader.parse(&mut buffer).unwrap();
        (loader, buffer)
    }

    /// Reads the rope from `byte` the way tree-sitter does, one piece after another.
    fn read_from(rope: &Rope, byte: usize) -> Vec<u8> {
        let mut text = vec![];
        loop {
            let piece = text_from(rope, byte + text.len());
            if piece.is_empty() {
                return text;
            }
            text.extend_from_slice(piece);
        }
    }

    #[test]
    fn highlights_name_the_text_they_cover() {
        let text = "fn main() { let s = \"hi\"; }";
        let (_, mut buffer) = rust_buffer(text);

        let found: Vec<_> = buffer
            .highlights(0..text.len())
            .map(|(bytes, highlight)| (&text[bytes], highlight))
            .collect();

        assert!(found.contains(&("fn", Highlight::Keyword)), "{found:?}");
        assert!(found.contains(&("main", Highlight::Function)), "{found:?}");
        assert!(found.contains(&("\"hi\"", Highlight::String)), "{found:?}");
    }

    #[test]
    fn highlights_only_touch_the_range() {
        let text = "fn a() {}\nfn b() {}\n";
        let (_, mut buffer) = rust_buffer(text);

        let names: Vec<_> = buffer
            .highlights(10..text.len())
            .map(|(bytes, _)| &text[bytes])
            .collect();

        assert!(names.contains(&"b"), "{names:?}");
        assert!(!names.contains(&"a"), "{names:?}");
    }

    #[test]
    fn an_edit_is_followed_once_the_buffer_is_parsed_again() {
        let (mut loader, mut buffer) = rust_buffer("fn a() {}");

        buffer.edit(3..4, "bb");
        loader.parse(&mut buffer).unwrap();

        let text = buffer.rope().to_string();
        let names: Vec<_> = buffer
            .highlights(0..text.len())
            .map(|(bytes, highlight)| (&text[bytes], highlight))
            .collect();
        assert!(
            names.contains(&("bb", Highlight::Function)),
            "the renamed function is highlighted whole, got {names:?}"
        );
    }

    #[test]
    fn a_buffer_with_no_parse_has_no_highlights() {
        let (_, grammar) = rust();
        let mut buffer = Buffer::from("fn a() {}");
        buffer.set_language(Some(LanguageTag::new("rust")));
        buffer.attach(&grammar);

        assert_eq!(
            buffer.highlights(0..9).count(),
            0,
            "the tree comes from the first parse"
        );
    }

    #[test]
    fn reading_on_gives_the_whole_text() {
        // Long enough for crop to split it over many chunks
        let text = "fn é() {}\n".repeat(2_000);
        let rope = Rope::from(text.as_str());
        for byte in [0, 1, 4, 5, 1_000, text.len() - 1] {
            assert_eq!(
                read_from(&rope, byte),
                &text.as_bytes()[byte..],
                "from byte {byte}"
            );
        }
    }

    #[test]
    fn a_byte_inside_a_char_starts_inside_it() {
        // é is bytes 1..3
        let rope = Rope::from("aéb");
        assert_eq!(
            read_from(&rope, 2),
            b"\xa9b",
            "no panic, the rest of é and then b"
        );
    }

    #[test]
    fn the_end_is_empty() {
        let rope = Rope::from("ab");
        assert_eq!(text_from(&rope, 2), b"", "at the end");
        assert_eq!(text_from(&rope, 10), b"", "past the end too");
        assert_eq!(text_from(&Rope::new(), 0), b"", "an empty rope");
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
