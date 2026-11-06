use std::{result, str};

use tree_sitter::wasmtime;

use crate::syntax::LanguageTag;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("loading the grammar from wasm failed")]
    Wasm(#[from] tree_sitter::WasmError),
    #[error("the wasm runtime failed")]
    WasmTime(#[from] wasmtime::Error),
    #[error("the parser rejected the grammar")]
    Language(#[from] tree_sitter::LanguageError),
    #[error("the highlights query is invalid")]
    Query(#[from] tree_sitter::QueryError),
    /// `Loader::new` gives the parser a store and `load` always puts it back, so this only
    /// happens if one of them is wrong.
    #[error("the parser has no wasm store")]
    MissingWasmStore,
    #[error("the highlights query is not valid utf-8")]
    InvalidUtf8(#[from] str::Utf8Error),
    /// `Parser::parse_with_options` gave no tree. With a language set and no progress
    /// callback that shouldn't happen, but a grammar trapping inside wasm could cause it.
    #[error("parsing {} stopped without a tree", .0.as_str())]
    ParseStopped(LanguageTag),
}

pub type Result<T> = result::Result<T, Error>;
