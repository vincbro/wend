use crate::{
    input::Mode,
    text_box::{Direction, Motion},
};

/// What a text box does with its text, its selection and its mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextBoxAction {
    Motion(Motion),
    Mode(Mode),
    Insert(char),
    /// Inserts the character of the key that ran it, so a binding can type through the config.
    /// It does nothing when run without a key.
    InsertTyped,
    Delete,
    DeleteSelection,
    OpenLine,
    /// Drops the selection down to the cursor, the end that moves.
    CollapseSelection,
    /// Selects the screen row the head is on, a wrapped line one row at a time.
    SelectRow,
    Revert,
    /// Opens the prompt that searches the text, which only a box that can be searched does.
    Find,
}

impl TextBoxAction {
    /// The name a config gives this action, none for a character that is typed. The match
    /// has no catch-all, so an action added without a name doesn't compile.
    #[must_use]
    pub const fn name(self) -> Option<&'static str> {
        Some(match self {
            Self::Motion(Motion::Horizontal(Direction::Backward)) => "left",
            Self::Motion(Motion::Horizontal(Direction::Forward)) => "right",
            Self::Motion(Motion::Vertical(Direction::Forward)) => "down",
            Self::Motion(Motion::Vertical(Direction::Backward)) => "up",
            Self::Motion(Motion::Word(Direction::Forward)) => "next_word",
            Self::Motion(Motion::Word(Direction::Backward)) => "previous_word",
            Self::Motion(Motion::Line(Direction::Backward)) => "line_start",
            Self::Motion(Motion::Line(Direction::Forward)) => "line_end",
            Self::Motion(Motion::Page(Direction::Forward)) => "page_down",
            Self::Motion(Motion::Page(Direction::Backward)) => "page_up",
            Self::Motion(Motion::File(Direction::Backward)) => "file_start",
            Self::Motion(Motion::File(Direction::Forward)) => "file_end",
            Self::Motion(Motion::Match(Direction::Forward)) => "next_match",
            Self::Motion(Motion::Match(Direction::Backward)) => "previous_match",
            Self::Mode(Mode::Normal) => "normal_mode",
            Self::Mode(Mode::Select) => "select_mode",
            Self::Mode(Mode::Insert) => "insert_mode",
            Self::Insert('\n') => "insert_newline",
            Self::Insert('\t') => "insert_tab",
            Self::Insert(_) => return None,
            Self::InsertTyped => "insert_char",
            Self::Delete => "delete",
            Self::DeleteSelection => "delete_selection",
            Self::OpenLine => "open_line",
            Self::CollapseSelection => "collapse_selection",
            Self::SelectRow => "select_row",
            Self::Revert => "revert",
            Self::Find => "find",
        })
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Motion(Motion::Horizontal(Direction::Backward)) => "one grapheme left",
            Self::Motion(Motion::Horizontal(Direction::Forward)) => "one grapheme right",
            Self::Motion(Motion::Vertical(Direction::Forward)) => "one row down",
            Self::Motion(Motion::Vertical(Direction::Backward)) => "one row up",
            Self::Motion(Motion::Word(Direction::Forward)) => "select the next word",
            Self::Motion(Motion::Word(Direction::Backward)) => "select the previous word",
            Self::Motion(Motion::Line(Direction::Backward)) => "start of the line",
            Self::Motion(Motion::Line(Direction::Forward)) => "end of the line",
            Self::Motion(Motion::Page(Direction::Forward)) => "last row on screen",
            Self::Motion(Motion::Page(Direction::Backward)) => "first row on screen",
            Self::Motion(Motion::File(Direction::Backward)) => "start of the file",
            Self::Motion(Motion::File(Direction::Forward)) => "end of the file",
            Self::Motion(Motion::Match(Direction::Forward)) => "select the next match",
            Self::Motion(Motion::Match(Direction::Backward)) => "select the previous match",
            Self::Mode(Mode::Normal) => "normal mode",
            Self::Mode(Mode::Select) => "select mode",
            Self::Mode(Mode::Insert) => "insert mode",
            Self::Insert('\n') => "a line break",
            Self::Insert('\t') => "a tab",
            Self::Insert(_) => "a character",
            Self::InsertTyped => "the character of the key",
            Self::Delete => "delete the grapheme before the cursor",
            Self::DeleteSelection => "delete the selection",
            Self::OpenLine => "open a line below",
            Self::CollapseSelection => "drop the selection down to the cursor",
            Self::SelectRow => "select the screen row the cursor is on",
            Self::Revert => "revert to the last save",
            Self::Find => "search the text",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "left" => Self::Motion(Motion::Horizontal(Direction::Backward)),
            "right" => Self::Motion(Motion::Horizontal(Direction::Forward)),
            "down" => Self::Motion(Motion::Vertical(Direction::Forward)),
            "up" => Self::Motion(Motion::Vertical(Direction::Backward)),
            "next_word" => Self::Motion(Motion::Word(Direction::Forward)),
            "previous_word" => Self::Motion(Motion::Word(Direction::Backward)),
            "line_start" => Self::Motion(Motion::Line(Direction::Backward)),
            "line_end" => Self::Motion(Motion::Line(Direction::Forward)),
            "page_down" => Self::Motion(Motion::Page(Direction::Forward)),
            "page_up" => Self::Motion(Motion::Page(Direction::Backward)),
            "file_start" => Self::Motion(Motion::File(Direction::Backward)),
            "file_end" => Self::Motion(Motion::File(Direction::Forward)),
            "next_match" => Self::Motion(Motion::Match(Direction::Forward)),
            "previous_match" => Self::Motion(Motion::Match(Direction::Backward)),
            "normal_mode" => Self::Mode(Mode::Normal),
            "select_mode" => Self::Mode(Mode::Select),
            "insert_mode" => Self::Mode(Mode::Insert),
            "insert_newline" => Self::Insert('\n'),
            "insert_tab" => Self::Insert('\t'),
            "insert_char" => Self::InsertTyped,
            "delete" => Self::Delete,
            "delete_selection" => Self::DeleteSelection,
            "open_line" => Self::OpenLine,
            "collapse_selection" => Self::CollapseSelection,
            "select_row" => Self::SelectRow,
            "revert" => Self::Revert,
            "find" => Self::Find,
            _ => return None,
        })
    }
}
