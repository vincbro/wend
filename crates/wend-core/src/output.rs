use std::path::Path;

use crop::Rope;

use crate::{frame::Frame, input::action::SearchKind, syntax::LanguageTag};

/// What the editor asks of the frontend.
#[derive(Debug)]
pub enum Output {
    /// Asks the frontend to write `rope` to `path`. Copying a rope only shares its nodes, so
    /// the core never copies the text to save it.
    Write {
        path: Box<Path>,
        rope: Rope,
    },
    /// Asks the frontend to read the file at `path`, which it answers with `Input::File`.
    Read {
        path: Box<Path>,
    },
    /// The screen to draw. Send it back with `Input::Drawn` once it is drawn, the editor
    /// has no other frames to draw into.
    Frame(Frame),
    /// Asks for the grammar of this language, which the frontend answers with
    /// `Input::Language`. Until it does, the text is shown as plain text.
    Language(LanguageTag),
    /// Asks the frontend to find what matches `query` of the kind `kind`, which it answers with
    /// `Input::SearchResults` carrying the same `id`. The editor counts the searches, and drops
    /// an answer to one that a newer search has replaced.
    Search {
        id: u64,
        kind: SearchKind,
        query: Box<str>,
    },
    Quit,
}
