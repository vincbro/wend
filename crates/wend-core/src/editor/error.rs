use std::sync::mpsc;

use crate::{buffer, output::Output};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("buffer operation failed")]
    Buffer(#[from] buffer::Error),
    #[error("failed to send output")]
    SendError(#[from] mpsc::SendError<Output>),
}
