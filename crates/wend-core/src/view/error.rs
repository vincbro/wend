use std::result;

use crate::buffer;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("buffer operation failed")]
    Buffer(#[from] buffer::Error),
}

pub type Result<T> = result::Result<T, Error>;
