use std::{fmt, path::Path};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, Visitor},
    ser,
};

mod overlay;
pub use overlay::{
    CommandAction, FindAction, OverlayAction, OverlayKind, SearchAction, SearchKind,
    TestOverlayAction,
};

use crate::{
    input::{self, Mode},
    text_box::{Direction, Motion},
};

/// What a key can do, as a group for each target that takes keys.
///
/// Not `Copy`, because an action that takes arguments owns them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Editor(EditorAction),
    Scratch(ScratchAction),
    TextBox(TextBoxAction),
    Overlay(OverlayAction),
}

/// What any target can do, wherever the keys came from. It needs nothing of a document or of
/// text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorAction {
    /// Asks the frontend for the file at the path, which the editor then shows.
    Open(Box<Path>),
    /// Asks the frontend for the file at the path, which the editor then shows with the cursor
    /// on the line, counted from 1.
    OpenAt {
        path: Box<Path>,
        line: usize,
    },
    /// Opens the overlay that tries out arranging two windows.
    OpenTestOverlay,
    /// Opens the command line, which runs the action its text names.
    OpenCommandLine,
    /// Opens the search, which lists what the frontend finds and opens the one picked.
    OpenSearch(SearchKind),
    /// Closes the overlay that has the keys.
    CloseOverlay,
    Quit,
}

/// What the scratch does about the document it shows, which a text box has no notion of.
///
/// Not `Copy`, because an action that takes arguments owns them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScratchAction {
    /// Writes the document to the path, or to its own file when there is none.
    Save(Option<Box<Path>>),
}

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

/// The groups of actions a piece of text can name, which is what a place that takes actions as
/// text accepts. The editor's group is in every scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Only what any target can do.
    Editor,
    /// What any target can do and what a text box does.
    TextBox,
    /// What any target can do, what a text box does and what the scratch does.
    Scratch,
    /// What any target can do and what the given overlay does.
    Overlay(OverlayKind),
}

impl Action {
    /// The name a config gives this action, none for a character that is typed.
    #[must_use]
    pub const fn name(&self) -> Option<&'static str> {
        match self {
            Self::Editor(action) => Some(action.name()),
            Self::Scratch(action) => Some(action.name()),
            Self::TextBox(action) => action.name(),
            Self::Overlay(action) => Some(action.name()),
        }
    }

    /// What the action does, in a few words for a config file to show next to a binding.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Editor(action) => action.description(),
            Self::Scratch(action) => action.description(),
            Self::TextBox(action) => action.description(),
            Self::Overlay(action) => action.description(),
        }
    }

    /// Reads the action `text` writes: the name of one in `scope`, then its arguments,
    /// which are the rest of the text. An action that takes none refuses any.
    pub fn parse(scope: Scope, text: &str) -> input::Result<Self> {
        let text = text.trim();
        let (name, arguments) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
        let arguments = arguments.trim();
        if let Some(action) = EditorAction::with_arguments(name, arguments) {
            return action.map(Self::Editor);
        }
        if let (Scope::Scratch, Some(action)) =
            (scope, ScratchAction::with_arguments(name, arguments))
        {
            return Ok(Self::Scratch(action));
        }
        let action = EditorAction::from_name(name)
            .map(Self::Editor)
            .or_else(|| match scope {
                Scope::Editor => None,
                Scope::TextBox => TextBoxAction::from_name(name).map(Self::TextBox),
                Scope::Scratch => ScratchAction::from_name(name)
                    .map(Self::Scratch)
                    .or_else(|| TextBoxAction::from_name(name).map(Self::TextBox)),
                Scope::Overlay(kind) => OverlayAction::from_name(kind, name).map(Self::Overlay),
            })
            .ok_or_else(|| input::Error::UnknownAction(name.into()))?;
        if !arguments.is_empty() {
            return Err(input::Error::UnexpectedArguments(name.into()));
        }
        Ok(action)
    }
}

impl EditorAction {
    /// The action with arguments that `name` means, none when `name` takes none.
    fn with_arguments(name: &str, arguments: &str) -> Option<input::Result<Self>> {
        match name {
            "open" | "o" if arguments.is_empty() => {
                Some(Err(input::Error::MissingArguments(name.into())))
            }
            "open" | "o" => Some(Ok(Self::Open(Path::new(arguments).into()))),
            "open_at" => Some(Self::open_at(arguments)),
            "open_search" if !arguments.is_empty() => Some(
                SearchKind::from_name(arguments)
                    .map(Self::OpenSearch)
                    .ok_or(input::Error::InvalidArguments {
                        name: "open_search",
                        expected: "`files` or `text`",
                    }),
            ),
            _ => None,
        }
    }

    /// Reads the arguments of `open_at`: a path, a colon and a line, counted from 1.
    fn open_at(arguments: &str) -> input::Result<Self> {
        let invalid = || input::Error::InvalidArguments {
            name: "open_at",
            expected: "a path and a line, as `path:line`",
        };
        let (path, line) = arguments.rsplit_once(':').ok_or_else(invalid)?;
        let line = line.parse().map_err(|_| invalid())?;
        Ok(Self::OpenAt {
            path: Path::new(path).into(),
            line,
        })
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Open(_) => "open",
            Self::OpenAt { .. } => "open_at",
            Self::OpenTestOverlay => "open_test_overlay",
            Self::OpenCommandLine => "open_command_line",
            Self::OpenSearch(_) => "open_search",
            Self::CloseOverlay => "close_overlay",
            Self::Quit => "quit",
        }
    }

    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Open(_) => "open a file",
            Self::OpenAt { .. } => "open a file at a line",
            Self::OpenTestOverlay => "two windows side by side",
            Self::OpenCommandLine => "type a command",
            Self::OpenSearch(SearchKind::Files) => "search for a file",
            Self::OpenSearch(SearchKind::Text) => "search for text in the files",
            Self::CloseOverlay => "close the overlay",
            Self::Quit => "quit",
        }
    }

    /// The action `name` means when it takes no arguments, the inverse of `name`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "open_test_overlay" => Self::OpenTestOverlay,
            "open_command_line" => Self::OpenCommandLine,
            "open_search" => Self::OpenSearch(SearchKind::Files),
            "close_overlay" => Self::CloseOverlay,
            "quit" | "q" => Self::Quit,
            _ => return None,
        })
    }
}

impl ScratchAction {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Save(_) => "save",
        }
    }

    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Save(_) => "save",
        }
    }

    /// The action with arguments that `name` means, none when there are no arguments.
    fn with_arguments(name: &str, arguments: &str) -> Option<Self> {
        match name {
            "save" | "write" | "s" | "w" if !arguments.is_empty() => {
                Some(Self::Save(Some(Path::new(arguments).into())))
            }
            _ => None,
        }
    }

    /// The action `name` means when it takes no arguments, the inverse of `name`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "save" | "write" | "s" | "w" => Self::Save(None),
            _ => return None,
        })
    }
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

impl Serialize for Action {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if let Self::Editor(EditorAction::Open(path)) = self {
            return serializer.collect_str(&format_args!("open {}", path.display()));
        }
        if let Self::Editor(EditorAction::OpenAt { path, line }) = self {
            return serializer.collect_str(&format_args!("open_at {}:{line}", path.display()));
        }
        if let Self::Editor(EditorAction::OpenSearch(SearchKind::Text)) = self {
            return serializer.serialize_str("open_search text");
        }
        if let Self::Scratch(ScratchAction::Save(Some(path))) = self {
            return serializer.collect_str(&format_args!("save {}", path.display()));
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
                Action::parse(Scope::Scratch, name).map_err(E::custom)
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
            Action::TextBox(TextBoxAction::Insert('\n')),
            Action::TextBox(TextBoxAction::Insert('\t')),
            Action::TextBox(TextBoxAction::InsertTyped),
            Action::TextBox(TextBoxAction::Delete),
            Action::TextBox(TextBoxAction::DeleteSelection),
            Action::TextBox(TextBoxAction::OpenLine),
            Action::TextBox(TextBoxAction::CollapseSelection),
            Action::TextBox(TextBoxAction::SelectRow),
            Action::TextBox(TextBoxAction::Revert),
            Action::Scratch(ScratchAction::Save(None)),
            Action::Editor(EditorAction::OpenTestOverlay),
            Action::Editor(EditorAction::OpenCommandLine),
            Action::Editor(EditorAction::OpenSearch(SearchKind::Files)),
            Action::TextBox(TextBoxAction::Find),
            Action::Editor(EditorAction::CloseOverlay),
            Action::Editor(EditorAction::Quit),
        ];
        for mode in [Mode::Normal, Mode::Select, Mode::Insert] {
            actions.push(Action::TextBox(TextBoxAction::Mode(mode)));
        }
        for direction in directions {
            actions.push(Action::TextBox(TextBoxAction::Motion(Motion::File(
                direction,
            ))));
            for motion in motions {
                actions.push(Action::TextBox(TextBoxAction::Motion(motion(direction))));
            }
        }
        actions
    }

    #[test]
    fn every_name_reads_back_as_its_action() {
        for action in all() {
            let name = action.name().expect("every bindable action has a name");
            assert_eq!(
                Action::parse(Scope::Scratch, name).unwrap(),
                action,
                "{name}"
            );
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
                Err(input::Error::UnknownAction(_))
            ),
            "a motion is the text box's"
        );
    }

    #[test]
    fn the_overlay_scope_names_the_editor_group_and_not_the_scratch_or_text_box() {
        let scope = Scope::Overlay(OverlayKind::Test);
        assert_eq!(
            Action::parse(scope, "quit").unwrap(),
            Action::Editor(EditorAction::Quit)
        );
        assert!(
            matches!(
                Action::parse(scope, "left"),
                Err(input::Error::UnknownAction(_))
            ),
            "a motion is the text box's"
        );
    }

    #[test]
    fn saving_is_in_the_scratch_scope_and_no_narrower_one() {
        assert_eq!(
            Action::parse(Scope::Scratch, "save").unwrap(),
            Action::Scratch(ScratchAction::Save(None))
        );
        for scope in [
            Scope::Editor,
            Scope::TextBox,
            Scope::Overlay(OverlayKind::Test),
        ] {
            assert!(
                matches!(
                    Action::parse(scope, "save"),
                    Err(input::Error::UnknownAction(_))
                ),
                "{scope:?} has no document to save"
            );
        }
        assert_eq!(
            Action::parse(Scope::TextBox, "left").unwrap(),
            Action::TextBox(TextBoxAction::Motion(Motion::Horizontal(
                Direction::Backward
            ))),
            "the text box scope names the text box's group"
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
    fn save_takes_the_rest_of_the_text_as_a_path_and_write_is_save() {
        assert_eq!(
            Action::parse(Scope::Scratch, "write  my file.txt ").unwrap(),
            Action::Scratch(ScratchAction::Save(Some(Path::new("my file.txt").into())))
        );
        assert_eq!(
            Action::parse(Scope::Scratch, "w").unwrap(),
            Action::Scratch(ScratchAction::Save(None)),
            "without a path it is the document's own"
        );
    }

    #[test]
    fn open_search_takes_what_it_looks_for() {
        assert_eq!(
            Action::parse(Scope::Scratch, "open_search").unwrap(),
            Action::Editor(EditorAction::OpenSearch(SearchKind::Files)),
            "files without an argument"
        );
        assert_eq!(
            Action::parse(Scope::Scratch, "open_search text").unwrap(),
            Action::Editor(EditorAction::OpenSearch(SearchKind::Text))
        );
        assert!(
            matches!(
                Action::parse(Scope::Scratch, "open_search nothing"),
                Err(input::Error::InvalidArguments { .. })
            ),
            "only the kinds there are"
        );
    }

    #[test]
    fn open_at_takes_a_path_and_a_line_after_its_last_colon() {
        assert_eq!(
            Action::parse(Scope::Scratch, "open_at src/a:b.rs:12").unwrap(),
            Action::Editor(EditorAction::OpenAt {
                path: Path::new("src/a:b.rs").into(),
                line: 12
            })
        );
        let error = Action::parse(Scope::Scratch, "open_at src/a.rs").unwrap_err();
        assert_eq!(
            error.to_string(),
            "`open_at` takes a path and a line, as `path:line`",
            "it says what it takes, not that it takes nothing"
        );
    }

    #[test]
    fn open_without_a_path_is_refused() {
        assert!(
            matches!(
                Action::parse(Scope::Scratch, "open"),
                Err(input::Error::MissingArguments(name)) if &*name == "open"
            ),
            "there is nothing to open"
        );
    }

    #[test]
    fn surrounding_space_is_ignored() {
        assert_eq!(
            Action::parse(Scope::Scratch, "  save  ").unwrap(),
            Action::Scratch(ScratchAction::Save(None))
        );
    }

    #[test]
    fn arguments_to_an_action_that_takes_none_are_refused() {
        assert!(
            matches!(
                Action::parse(Scope::Scratch, "quit now"),
                Err(input::Error::UnexpectedArguments(name)) if &*name == "quit"
            ),
            "quit has no arguments"
        );
    }

    #[test]
    fn a_typed_character_has_no_name() {
        assert_eq!(Action::TextBox(TextBoxAction::Insert('x')).name(), None);
    }
}
