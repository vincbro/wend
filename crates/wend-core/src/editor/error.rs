use std::{result, sync::mpsc};

use crate::{buffer, output::Output, syntax};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("buffer operation failed")]
    Buffer(#[from] buffer::Error),
    #[error("failed to send output")]
    Send(#[from] mpsc::SendError<Output>),
    #[error("")]
    Syntax(#[from] syntax::Error),
}

pub type Result<T> = result::Result<T, Error>;
