use crate::{editor::Mode, view::Motion};

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Motion(Motion),
    Mode(Mode),
    Insert(char),
    Delete,
    DeleteSelection,
    OpenLine,
    Revert,
    Save,
    Quit,
}
