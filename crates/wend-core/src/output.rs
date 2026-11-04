use std::path::PathBuf;

use crop::Rope;

use crate::frame::Frame;

#[derive(Debug)]
pub enum Output {
    /// Copying a rope only shares its nodes, so the core never copies the text to save it.
    Write {
        path: PathBuf,
        rope: Rope,
    },
    Frame(Frame),
    Quit,
}
