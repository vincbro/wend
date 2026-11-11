use std::result;

use crate::syntax;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("file is not valid utf-8")]
    InvalidUtf8,
    #[error("buffer has no path to save to")]
    NoPath,
    #[error("failed to parse the text")]
    Syntax(#[from] syntax::Error),
}

pub type Result<T> = result::Result<T, Error>;
