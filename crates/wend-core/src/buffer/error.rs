use std::{io, result};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("file is not valid utf-8")]
    InvalidUtf8,
    #[error("buffer has no path to save to")]
    NoPath,
    #[error("file i/o failed")]
    Io(#[from] io::Error),
}

pub type Result<T> = result::Result<T, Error>;
