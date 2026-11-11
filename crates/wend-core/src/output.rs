use std::path::Path;

use crop::Rope;

use crate::{frame::Frame, syntax::LanguageTag};

/// What the editor asks of the frontend.
#[derive(Debug)]
pub enum Output {
    /// Asks the frontend to write `rope` to `path`. Copying a rope only shares its nodes, so
    /// the core never copies the text to save it.
    Write {
        path: Box<Path>,
        rope: Rope,
    },
    /// The screen to draw. Send it back with `Input::Drawn` once it is drawn, the editor
    /// has no other frames to draw into.
    Frame(Frame),
    /// Asks for the grammar of this language, which the frontend answers with
    /// `Input::Language`. Until it does, the text is shown as plain text.
    Language(LanguageTag),
    Quit,
}
