use core::str;

use tree_sitter::wasmtime;

use crate::syntax::LanguageTag;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("")]
    Wasm(#[from] tree_sitter::WasmError),
    #[error("")]
    WasmTime(#[from] wasmtime::Error),
    #[error("")]
    Language(#[from] tree_sitter::LanguageError),
    #[error("")]
    Query(#[from] tree_sitter::QueryError),
    #[error("")]
    MissingLanguage(LanguageTag),
    #[error("")]
    MissingWasmStore,
    #[error("")]
    InvalidUtf8(#[from] str::Utf8Error),
    /// `Parser::parse_with_options` gave no tree. With a language set and no progress
    /// callback that shouldn't happen, but a grammar trapping inside wasm could cause it.
    #[error("parsing {} stopped without a tree", .0.as_str())]
    ParseStopped(LanguageTag),
}
