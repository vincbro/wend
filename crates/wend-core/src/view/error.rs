use std::result;

use crate::{buffer, config};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("buffer operation failed")]
    Buffer(#[from] buffer::Error),
    #[error("config is invalid")]
    Config(#[from] config::Error),
}

pub type Result<T> = result::Result<T, Error>;
