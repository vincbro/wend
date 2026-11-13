use std::fmt;

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, Visitor},
    ser,
};

use crate::{
    config::Error,
    input::Mode,
    view::{Direction, Motion},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Motion(Motion),
    Mode(Mode),
    Insert(char),
    Delete,
    DeleteSelection,
    OpenLine,
    /// Drops the selection down to the cursor, the end that moves.
    CollapseSelection,
    /// Selects the screen row the head is on, a wrapped line one row at a time.
    SelectRow,
    Revert,
    Save,
    Quit,
}

impl Action {
    /// The name a config gives this action, none for a character typing itself. The match
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
            Self::Mode(Mode::Normal) => "normal_mode",
            Self::Mode(Mode::Select) => "select_mode",
            Self::Mode(Mode::Insert) => "insert_mode",
            Self::Insert('\n') => "insert_newline",
            Self::Insert('\t') => "insert_tab",
            Self::Insert(_) => return None,
            Self::Delete => "delete",
            Self::DeleteSelection => "delete_selection",
            Self::OpenLine => "open_line",
            Self::CollapseSelection => "collapse_selection",
            Self::SelectRow => "select_row",
            Self::Revert => "revert",
            Self::Save => "save",
            Self::Quit => "quit",
        })
    }

    /// What the action does, in a few words for a config file to show next to a binding.
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
            Self::Mode(Mode::Normal) => "normal mode",
            Self::Mode(Mode::Select) => "select mode",
            Self::Mode(Mode::Insert) => "insert mode",
            Self::Insert('\n') => "a line break",
            Self::Insert('\t') => "a tab",
            Self::Insert(_) => "a character",
            Self::Delete => "delete the grapheme before the cursor",
            Self::DeleteSelection => "delete the selection",
            Self::OpenLine => "open a line below",
            Self::CollapseSelection => "drop the selection down to the cursor",
            Self::SelectRow => "select the screen row the cursor is on",
            Self::Revert => "revert to the last save",
            Self::Save => "save",
            Self::Quit => "quit",
        }
    }

    /// The action a config means by `name`, the inverse of `name`.
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
            "normal_mode" => Self::Mode(Mode::Normal),
            "select_mode" => Self::Mode(Mode::Select),
            "insert_mode" => Self::Mode(Mode::Insert),
            "insert_newline" => Self::Insert('\n'),
            "insert_tab" => Self::Insert('\t'),
            "delete" => Self::Delete,
            "delete_selection" => Self::DeleteSelection,
            "open_line" => Self::OpenLine,
            "collapse_selection" => Self::CollapseSelection,
            "select_row" => Self::SelectRow,
            "revert" => Self::Revert,
            "save" => Self::Save,
            "quit" => Self::Quit,
            _ => return None,
        })
    }
}

impl Serialize for Action {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let name = self
            .name()
            .ok_or_else(|| ser::Error::custom(format!("{self:?} has no name to bind")))?;
        serializer.serialize_str(name)
    }
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ActionVisitor;

        impl Visitor<'_> for ActionVisitor {
            type Value = Action;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("the name of an action")
            }

            fn visit_str<E: de::Error>(self, name: &str) -> Result<Action, E> {
                Action::from_name(name).ok_or_else(|| E::custom(Error::UnknownAction(name.into())))
            }
        }

        deserializer.deserialize_str(ActionVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every action there is, so `from_name` can be held to `name`.
    fn all() -> Vec<Action> {
        let directions = [Direction::Backward, Direction::Forward];
        let motions: [fn(Direction) -> Motion; 5] = [
            Motion::Horizontal,
            Motion::Vertical,
            Motion::Word,
            Motion::Line,
            Motion::Page,
        ];
        let mut actions = vec![
            Action::Insert('\n'),
            Action::Insert('\t'),
            Action::Delete,
            Action::DeleteSelection,
            Action::OpenLine,
            Action::CollapseSelection,
            Action::SelectRow,
            Action::Revert,
            Action::Save,
            Action::Quit,
        ];
        for mode in [Mode::Normal, Mode::Select, Mode::Insert] {
            actions.push(Action::Mode(mode));
        }
        for direction in directions {
            actions.push(Action::Motion(Motion::File(direction)));
            for motion in motions {
                actions.push(Action::Motion(motion(direction)));
            }
        }
        actions
    }

    #[test]
    fn every_name_reads_back_as_its_action() {
        for action in all() {
            let name = action.name().expect("every bindable action has a name");
            assert_eq!(Action::from_name(name), Some(action), "{name}");
        }
    }

    #[test]
    fn a_typed_character_has_no_name() {
        assert_eq!(Action::Insert('x').name(), None);
    }
}
