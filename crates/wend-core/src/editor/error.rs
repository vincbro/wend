use std::result;

use crate::{buffer, syntax, view};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("buffer operation failed")]
    Buffer(#[from] buffer::Error),
    #[error("view operation failed")]
    View(#[from] view::Error),
    #[error("syntax failed")]
    Syntax(#[from] syntax::Error),
}

pub type Result<T> = result::Result<T, Error>;
