use std::path::PathBuf;

use crate::{
    frame::{Area, Frame},
    input::{action::Action, key::Key},
};

pub mod action;
pub mod key;

#[derive(Debug, Clone)]
pub enum Input {
    Action(Action),
    Resize(Area),
    Key(Key),
    File { path: PathBuf, bytes: Box<[u8]> },
    Drawn(Frame),
}
