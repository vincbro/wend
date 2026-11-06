use std::{collections::HashMap, fmt, ops::Range, sync::Arc};

use crop::Rope;
use tree_sitter::{
    Language, Node, Parser, Query, QueryCursor, StreamingIterator, Tree, WasmStore,
    wasmtime::{self, Engine},
};

mod error;
pub use error::{Error, Result};
mod highlight;
pub use highlight::Highlight;

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

struct Grammar {
    tag: LanguageTag,
    language: Language,
    highlights: Query,
    /// What each of the query's captures highlights as, by capture index.
    captures: Box<[Option<Highlight>]>,
}

/// Parses text and finds what to highlight in it, for every language whose grammar was loaded.
///
/// Parsing and querying both need the parser and cursor to themselves, so they take `&mut self`.
pub struct Syntax {
    parser: Parser,
    grammars: HashMap<LanguageTag, Arc<Grammar>>,
    cursor: QueryCursor,
}

impl Syntax {
    pub fn new() -> Result<Self> {
        let engine = Engine::new(&wasmtime::Config::new())?;
        let store = WasmStore::new(&engine)?;
        let mut parser = Parser::new();
        parser.set_wasm_store(store)?;
        Ok(Self {
            parser,
            grammars: HashMap::new(),
            cursor: QueryCursor::new(),
        })
    }

    /// Loads the grammar of `tag` from its compiled wasm and the source of its highlights
    /// query, replacing the grammar that was loaded for `tag` before.
    ///
    /// Fails when the wasm isn't a grammar this tree-sitter can run, or when the query
    /// doesn't fit the grammar.
    pub fn load(&mut self, tag: LanguageTag, wasm: &[u8], highlights: &[u8]) -> Result<()> {
        let mut store = self
            .parser
            .take_wasm_store()
            .ok_or(Error::MissingWasmStore)?;
        let language = store.load_language(tag.as_str(), wasm)?;
        self.parser.set_wasm_store(store)?;

        let highlights = str::from_utf8(highlights)?;
        let highlights = Query::new(&language, highlights)?;
        let captures = highlights
            .capture_names()
            .iter()
            .map(|name| Highlight::from_capture(name))
            .collect();

        self.grammars.insert(
            tag.clone(),
            Grammar {
                tag,
                language,
                highlights,
                captures,
            }
            .into(),
        );
        Ok(())
    }

    #[must_use]
    pub fn exists(&self, tag: &LanguageTag) -> bool {
        self.grammars.contains_key(tag)
    }

    /// Parses `rope` with the grammar of `language`, reading the rope chunk by chunk so the
    /// text is never copied. `old` must already be edited to match the rope with
    /// `Tree::edit`, then the parse reuses every part of it the edits didn't touch.
    pub fn parse(&mut self, tag: &LanguageTag, rope: &Rope, old: Option<&Tree>) -> Result<Tree> {
        let Some(grammar) = self.grammars.get(tag) else {
            return Err(Error::MissingLanguage(tag.clone()));
        };
        self.parser.set_language(&grammar.language)?;

        self.parser
            .parse_with_options(&mut |byte, _| text_from(rope, byte), old, None)
            .ok_or_else(|| Error::ParseStopped(tag.clone()))
    }

    /// The highlighted stretches of `rope` that touch `bytes`, in the order they start.
    /// `tree` must be the tree `parse` gave for the current text.
    ///
    /// Stretches nest and overlap, an escape inside a string for one, and the one that
    /// comes later is the more specific. The first ones can start before `bytes`.
    pub fn highlights<'a>(
        &'a mut self,
        tag: &LanguageTag,
        tree: &'a Tree,
        rope: &'a Rope,
        bytes: Range<usize>,
    ) -> Result<impl Iterator<Item = (Range<usize>, Highlight)> + 'a> {
        let Some(grammar) = self.grammars.get(tag) else {
            return Err(Error::MissingLanguage(tag.clone()));
        };
        self.cursor.set_byte_range(bytes);
        let text = |node: Node<'_>| {
            rope.byte_slice(node.byte_range())
                .chunks()
                .map(str::as_bytes)
        };
        // Captures without a highlight, like the ones a query only uses in predicates, are
        // skipped
        let highlights = self
            .cursor
            .captures(&grammar.highlights, tree.root_node(), text)
            .filter_map(move |(query_match, index)| {
                let capture = query_match.captures()[*index];
                let highlight = grammar.captures[capture.index as usize]?;
                Some((capture.node.byte_range(), highlight))
            })
            .cloned();
        Ok(highlights)
    }
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

impl fmt::Debug for Syntax {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Syntax")
            .field("languages", &self.grammars.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn rust_syntax() -> Syntax {
        let mut syntax = Syntax::new().unwrap();
        syntax
            .load(
                LanguageTag::new("rust"),
                include_bytes!("../../../tree-sitter-rust.wasm"),
                include_bytes!("../../../highlights.scm"),
            )
            .unwrap();
        syntax
    }

    #[test]
    fn highlights_name_the_text_they_cover() {
        let mut syntax = rust_syntax();
        let tag = LanguageTag::new("rust");
        let text = "fn main() { let s = \"hi\"; }";
        let rope = Rope::from(text);
        let tree = syntax.parse(&tag, &rope, None).unwrap();

        let found: Vec<_> = syntax
            .highlights(&tag, &tree, &rope, 0..text.len())
            .unwrap()
            .map(|(bytes, highlight)| (&text[bytes], highlight))
            .collect();

        assert!(found.contains(&("fn", Highlight::Keyword)), "{found:?}");
        assert!(found.contains(&("main", Highlight::Function)), "{found:?}");
        assert!(found.contains(&("\"hi\"", Highlight::String)), "{found:?}");
    }

    #[test]
    fn highlights_only_touch_the_range() {
        let mut syntax = rust_syntax();
        let tag = LanguageTag::new("rust");
        let text = "fn a() {}\nfn b() {}\n";
        let rope = Rope::from(text);
        let tree = syntax.parse(&tag, &rope, None).unwrap();

        let second_line = 10..text.len();
        let names: Vec<_> = syntax
            .highlights(&tag, &tree, &rope, second_line)
            .unwrap()
            .map(|(bytes, _)| &text[bytes])
            .collect();

        assert!(names.contains(&"b"), "{names:?}");
        assert!(!names.contains(&"a"), "{names:?}");
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
}
