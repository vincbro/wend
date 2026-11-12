use std::result;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("a key combo can't be empty")]
    EmptyChord,
    #[error("`<` is never closed in `{0}`")]
    UnclosedBracket(Box<str>),
    #[error("unknown key `{0}`")]
    UnknownKey(Box<str>),
    #[error("`{0}` shifts a character, write the shifted character instead")]
    ShiftedCharacter(Box<str>),
    #[error("unknown action `{0}`")]
    UnknownAction(Box<str>),
    #[error("the leader can't be `<leader>`")]
    LeaderIsLeader,
    #[error("in {mode} mode, `{chord}` is bound as a key and also as the start of a longer combo")]
    Conflict { mode: &'static str, chord: Box<str> },
}

pub type Result<T> = result::Result<T, Error>;
