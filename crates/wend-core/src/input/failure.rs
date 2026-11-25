use std::path::Path;

use crate::syntax::LanguageTag;

/// What the frontend failed to do, which the editor shows to the user and recovers from.
///
/// The reason is the frontend's own words for it, since only it knows why.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Failure {
    /// The file the editor asked to read, with `Output::Read`, could not be read.
    #[error("failed to read {}: {reason}", path.display())]
    Read { path: Box<Path>, reason: Box<str> },
    /// The file the editor asked to write, with `Output::Write`, could not be written.
    #[error("failed to write {}: {reason}", path.display())]
    Write { path: Box<Path>, reason: Box<str> },
    /// The grammar the editor asked for, with `Output::Language`, could not be found.
    #[error("no grammar for {}: {reason}", tag.as_str())]
    Language { tag: LanguageTag, reason: Box<str> },
}
