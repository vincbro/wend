use std::result;

use crate::{buffer, config, syntax, view};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("buffer operation failed")]
    Buffer(#[from] buffer::Error),
    #[error("view operation failed")]
    View(#[from] view::Error),
    #[error("config is invalid")]
    Config(#[from] config::Error),
    #[error("syntax failed")]
    Syntax(#[from] syntax::Error),
}

pub type Result<T> = result::Result<T, Error>;
