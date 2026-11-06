use std::{collections::HashMap, fmt, sync::Arc};

use tree_sitter::{
    Parser, Query, WasmStore,
    wasmtime::{self, Engine},
};

use super::{Error, Grammar, Highlight, LanguageTag, Result};
use crate::buffer::Buffer;

/// Loads grammars and parses with them. It is the one parser that every buffer's tree comes
/// from, and it knows every grammar it loaded.
pub struct Loader {
    parser: Parser,
    grammars: HashMap<LanguageTag, Arc<Grammar>>,
}

impl Loader {
    pub fn new() -> Result<Self> {
        let engine = Engine::new(&wasmtime::Config::new())?;
        let store = WasmStore::new(&engine)?;
        let mut parser = Parser::new();
        parser.set_wasm_store(store)?;
        Ok(Self {
            parser,
            grammars: HashMap::new(),
        })
    }

    /// Loads the grammar of `tag` from its compiled wasm and the source of its highlights
    /// query, replacing the grammar that was loaded for `tag` before. Buffers keep the grammar
    /// they were given until they are given the new one.
    ///
    /// Fails when the wasm isn't a grammar this tree-sitter can run, or when the query
    /// doesn't fit the grammar. A failed load leaves the loader as it was.
    pub fn load(
        &mut self,
        tag: LanguageTag,
        wasm: &[u8],
        highlights: &[u8],
    ) -> Result<Arc<Grammar>> {
        let mut store = self
            .parser
            .take_wasm_store()
            .ok_or(Error::MissingWasmStore)?;
        let language = store.load_language(tag.as_str(), wasm);
        // Put back before the result is looked at, so a bad grammar can't take the store
        // with it
        self.parser.set_wasm_store(store)?;
        let language = language?;

        let source = str::from_utf8(highlights)?;
        let query = Query::new(&language, source)?;
        let captures = query
            .capture_names()
            .iter()
            .map(|name| Highlight::from_capture(name))
            .collect();

        let grammar = Arc::new(Grammar {
            tag: tag.clone(),
            language,
            query,
            captures,
        });
        self.grammars.insert(tag, Arc::clone(&grammar));
        Ok(grammar)
    }

    /// Returns the grammar loaded for `tag`, none until it was loaded.
    #[must_use]
    pub fn grammar(&self, tag: &LanguageTag) -> Option<&Arc<Grammar>> {
        self.grammars.get(tag)
    }

    /// Brings the tree of `buffer` up to date with its text. Does nothing for a buffer with no
    /// syntax, or whose tree already is.
    pub fn parse(&mut self, buffer: &mut Buffer) -> Result<()> {
        // Copying a rope only shares its nodes. It lets the syntax be borrowed from the buffer
        // while the text is read.
        let rope = buffer.rope().clone();
        let Some(syntax) = buffer.syntax_mut() else {
            return Ok(());
        };
        syntax.parse(&mut self.parser, &rope)
    }
}

impl fmt::Debug for Loader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Loader")
            .field("languages", &self.grammars.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::tests;

    #[test]
    fn a_failed_load_leaves_the_loader_working() {
        let mut loader = Loader::new().unwrap();
        let tag = LanguageTag::new("rust");
        let wasm = tests::RUST_WASM;
        let highlights = tests::RUST_HIGHLIGHTS;

        assert!(
            loader.load(tag.clone(), b"not wasm", highlights).is_err(),
            "bytes that aren't a grammar are an error"
        );
        assert!(
            loader.load(tag.clone(), wasm, b"(not a query").is_err(),
            "a query that doesn't parse is an error"
        );
        assert!(
            loader.grammar(&tag).is_none(),
            "nothing is remembered of a failed load"
        );

        let grammar = loader.load(tag.clone(), wasm, highlights).unwrap();
        let mut buffer = Buffer::from("fn a() {}");
        buffer.set_language(Some(tag));
        buffer.attach(&grammar);
        loader
            .parse(&mut buffer)
            .expect("the wasm store survived the failed loads");
    }

    #[test]
    fn plain_text_parses_to_nothing() {
        let (mut loader, _) = tests::rust();
        let mut buffer = Buffer::from("fn a() {}");
        loader.parse(&mut buffer).unwrap();
        assert_eq!(buffer.highlights(0..9).count(), 0, "no syntax, no colors");
    }
}
