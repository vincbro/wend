use std::{error::Error as StdError, result};

use crate::{buffer, config, input::Failure, syntax, text_box};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("buffer operation failed")]
    Buffer(#[from] buffer::Error),
    #[error("text box operation failed")]
    TextBox(#[from] text_box::Error),
    #[error("config is invalid")]
    Config(#[from] config::Error),
    /// What a command typed in the command line got wrong, which is all there is to say.
    #[error(transparent)]
    Command(config::Error),
    #[error("the file shown has no path to save to")]
    NoPath,
    #[error(transparent)]
    Frontend(#[from] Failure),
    #[error("syntax failed")]
    Syntax(#[from] syntax::Error),
}

impl Error {
    /// Returns what a message says: the error and what caused it, since an error alone often
    /// only says what was being done.
    pub(super) fn describe(&self) -> String {
        let mut text = self.to_string();
        let mut source = self.source();
        while let Some(cause) = source {
            text.push_str(": ");
            text.push_str(&cause.to_string());
            source = cause.source();
        }
        text
    }
}

pub type Result<T> = result::Result<T, Error>;
