use std::path::PathBuf;

use crop::Rope;

use crate::{frame::Frame, syntax::LanguageTag};

/// What the editor asks of the frontend.
#[derive(Debug)]
pub enum Output {
    /// Copying a rope only shares its nodes, so the core never copies the text to save it.
    Write {
        path: PathBuf,
        rope: Rope,
    },
    /// The screen to draw. Send it back with `Input::Drawn` once it is drawn, the editor
    /// has no other frames to draw into.
    Frame(Frame),
    Language(LanguageTag),
    Quit,
}
