use std::{fmt, path::Path};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, Visitor},
    ser,
};

mod overlay;
pub use overlay::{OverlayAction, OverlayKind, TestOverlayAction};

use crate::{
    config::{self, Error},
    input::Mode,
    view::{Direction, Motion},
};

/// What a key can do, as a group for each target that takes keys.
///
/// Not `Copy`, because an action that takes arguments owns them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Editor(EditorAction),
    View(ViewAction),
    Overlay(OverlayAction),
}

/// What any target can do, wherever the keys came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorAction {
    Save,
    /// Asks the frontend for the file at the path, which the editor then shows.
    Open(Box<Path>),
    /// Opens the overlay that tries out arranging two windows.
    OpenTestOverlay,
    /// Closes the overlay that has the keys.
    CloseOverlay,
    Quit,
}

/// What a view does with its text, its selection and its mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewAction {
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
}

/// The groups of actions a piece of text can name, which is what a place that takes actions as
/// text accepts. The editor's group is in every scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Only what any target can do.
    Editor,
    /// What any target can do and what a view does.
    View,
    /// What any target can do and what the given overlay does.
    Overlay(OverlayKind),
}

impl Action {
    /// The name a config gives this action, none for a character typing itself.
    #[must_use]
    pub const fn name(&self) -> Option<&'static str> {
        match self {
            Self::Editor(action) => Some(action.name()),
            Self::View(action) => action.name(),
            Self::Overlay(action) => Some(action.name()),
        }
    }

    /// What the action does, in a few words for a config file to show next to a binding.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Editor(action) => action.description(),
            Self::View(action) => action.description(),
            Self::Overlay(action) => action.description(),
        }
    }

    /// Reads the action `text` writes: the name of one in `scope`, then its arguments,
    /// which are the rest of the text. An action that takes none refuses any.
    pub fn parse(scope: Scope, text: &str) -> config::Result<Self> {
        let text = text.trim();
        let (name, arguments) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
        let arguments = arguments.trim();
        if let Some(action) = EditorAction::with_arguments(name, arguments) {
            return action.map(Self::Editor);
        }
        let action = EditorAction::from_name(name)
            .map(Self::Editor)
            .or_else(|| match scope {
                Scope::Editor => None,
                Scope::View => ViewAction::from_name(name).map(Self::View),
                Scope::Overlay(kind) => OverlayAction::from_name(kind, name).map(Self::Overlay),
            })
            .ok_or_else(|| Error::UnknownAction(name.into()))?;
        if !arguments.is_empty() {
            return Err(Error::UnexpectedArguments(name.into()));
        }
        Ok(action)
    }
}

impl EditorAction {
    /// The action with arguments that `name` means, none when `name` takes none.
    fn with_arguments(name: &str, arguments: &str) -> Option<config::Result<Self>> {
        match name {
            "open" if arguments.is_empty() => Some(Err(Error::MissingArguments(name.into()))),
            "open" => Some(Ok(Self::Open(Path::new(arguments).into()))),
            _ => None,
        }
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Save => "save",
            Self::Open(_) => "open",
            Self::OpenTestOverlay => "open_test_overlay",
            Self::CloseOverlay => "close_overlay",
            Self::Quit => "quit",
        }
    }

    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Save => "save",
            Self::Open(_) => "open a file",
            Self::OpenTestOverlay => "two windows side by side",
            Self::CloseOverlay => "close the overlay",
            Self::Quit => "quit",
        }
    }

    /// The action `name` means when it takes no arguments, the inverse of `name`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "save" => Self::Save,
            "open_test_overlay" => Self::OpenTestOverlay,
            "close_overlay" => Self::CloseOverlay,
            "quit" => Self::Quit,
            _ => return None,
        })
    }
}

impl ViewAction {
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
            _ => return None,
        })
    }
}

impl Serialize for Action {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if let Self::Editor(EditorAction::Open(path)) = self {
            return serializer.collect_str(&format_args!("open {}", path.display()));
        }
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
                Action::parse(Scope::View, name).map_err(E::custom)
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
            Action::View(ViewAction::Insert('\n')),
            Action::View(ViewAction::Insert('\t')),
            Action::View(ViewAction::Delete),
            Action::View(ViewAction::DeleteSelection),
            Action::View(ViewAction::OpenLine),
            Action::View(ViewAction::CollapseSelection),
            Action::View(ViewAction::SelectRow),
            Action::View(ViewAction::Revert),
            Action::Editor(EditorAction::Save),
            Action::Editor(EditorAction::OpenTestOverlay),
            Action::Editor(EditorAction::CloseOverlay),
            Action::Editor(EditorAction::Quit),
        ];
        for mode in [Mode::Normal, Mode::Select, Mode::Insert] {
            actions.push(Action::View(ViewAction::Mode(mode)));
        }
        for direction in directions {
            actions.push(Action::View(ViewAction::Motion(Motion::File(direction))));
            for motion in motions {
                actions.push(Action::View(ViewAction::Motion(motion(direction))));
            }
        }
        actions
    }

    #[test]
    fn every_name_reads_back_as_its_action() {
        for action in all() {
            let name = action.name().expect("every bindable action has a name");
            assert_eq!(Action::parse(Scope::View, name).unwrap(), action, "{name}");
        }
    }

    #[test]
    fn the_editor_scope_names_only_the_editor_group() {
        assert_eq!(
            Action::parse(Scope::Editor, "quit").unwrap(),
            Action::Editor(EditorAction::Quit)
        );
        assert!(
            matches!(
                Action::parse(Scope::Editor, "left"),
                Err(Error::UnknownAction(_))
            ),
            "a motion is the view's"
        );
    }

    #[test]
    fn the_overlay_scope_names_the_editor_group_and_not_the_views() {
        let scope = Scope::Overlay(OverlayKind::Test);
        assert_eq!(
            Action::parse(scope, "quit").unwrap(),
            Action::Editor(EditorAction::Quit)
        );
        assert!(
            matches!(Action::parse(scope, "left"), Err(Error::UnknownAction(_))),
            "a motion is the view's"
        );
    }

    #[test]
    fn open_takes_the_rest_of_the_text_as_its_path() {
        assert_eq!(
            Action::parse(Scope::Editor, "open  src/my file.rs ").unwrap(),
            Action::Editor(EditorAction::Open(Path::new("src/my file.rs").into())),
            "spaces inside the path stay"
        );
    }

    #[test]
    fn open_without_a_path_is_refused() {
        assert!(
            matches!(
                Action::parse(Scope::View, "open"),
                Err(Error::MissingArguments(name)) if &*name == "open"
            ),
            "there is nothing to open"
        );
    }

    #[test]
    fn surrounding_space_is_ignored() {
        assert_eq!(
            Action::parse(Scope::View, "  save  ").unwrap(),
            Action::Editor(EditorAction::Save)
        );
    }

    #[test]
    fn arguments_to_an_action_that_takes_none_are_refused() {
        assert!(
            matches!(
                Action::parse(Scope::View, "save now"),
                Err(Error::UnexpectedArguments(name)) if &*name == "save"
            ),
            "save has no arguments"
        );
    }

    #[test]
    fn a_typed_character_has_no_name() {
        assert_eq!(Action::View(ViewAction::Insert('x')).name(), None);
    }
}
