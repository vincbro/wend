use std::result;

/// What reading an action from text can get wrong.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unknown action `{0}`")]
    UnknownAction(Box<str>),
    #[error("`{0}` takes no arguments")]
    UnexpectedArguments(Box<str>),
    #[error("`{0}` needs arguments")]
    MissingArguments(Box<str>),
    #[error("`{name}` takes {expected}")]
    InvalidArguments {
        name: &'static str,
        expected: &'static str,
    },
}

pub type Result<T> = result::Result<T, Error>;
