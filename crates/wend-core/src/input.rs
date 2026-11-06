use std::path::PathBuf;

use crate::{
    frame::{Area, Frame},
    input::{action::Action, key::Key},
    syntax::LanguageTag,
};

pub mod action;
pub mod key;

/// Everything that reaches the editor from outside: user actions, the terminal and files.
#[derive(Debug, Clone)]
pub enum Input {
    Action(Action),
    Resize(Area),
    Key(Key),
    /// What the file at `path` holds now, whether the editor asked for it or it changed.
    File {
        path: PathBuf,
        bytes: Box<[u8]>,
        language: Option<LanguageTag>,
    },
    /// A frame from `Output::Frame`, handed back once it is on screen so the editor can
    /// draw into it again.
    Drawn(Frame),
    Language {
        tag: LanguageTag,
        language: Box<[u8]>,
        highlights: Box<[u8]>,
    },
}
