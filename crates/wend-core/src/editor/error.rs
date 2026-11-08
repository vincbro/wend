use std::result;

use crate::{buffer, syntax};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("buffer operation failed")]
    Buffer(#[from] buffer::Error),
    #[error("syntax failed")]
    Syntax(#[from] syntax::Error),
}

pub type Result<T> = result::Result<T, Error>;
