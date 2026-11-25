use std::path::Path;

use crate::{
    config::Config,
    frame::{Area, Frame},
    input::{action::Action, key::Key},
    syntax::LanguageTag,
};

pub mod action;
mod failure;
pub use failure::Failure;
pub mod key;
mod mode;
pub use mode::Mode;

/// Everything that reaches the editor from outside: user actions, the terminal and files.
#[derive(Debug, Clone)]
pub enum Input {
    Action(Action),
    Resize(Area),
    Key(Key),
    /// What the file at `path` holds now, whether the editor asked for it or it changed.
    File {
        path: Box<Path>,
        bytes: Box<[u8]>,
        language: Option<LanguageTag>,
    },
    /// A frame from `Output::Frame`, handed back once it is on screen so the editor can
    /// draw into it again.
    Drawn(Frame),
    /// The grammar for `tag`, as the frontend's answer to `Output::Language`. `wasm` is the
    /// compiled tree-sitter grammar and `highlights` the source of its highlights query.
    Language {
        tag: LanguageTag,
        wasm: Box<[u8]>,
        highlights: Box<[u8]>,
    },
    /// A new config, read by the frontend from wherever it is kept, to replace the one the
    /// editor has.
    Config(Box<Config>),
    /// Something the frontend was asked to do and could not. The editor says so in the status bar
    /// and goes on, and a frontend reports every failure this way, not by exiting.
    Failed(Failure),
}
