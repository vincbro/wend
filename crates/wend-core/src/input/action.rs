use std::fmt;

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, Visitor},
    ser,
};

mod editor;
pub use editor::EditorAction;
mod overlay;
pub use overlay::{
    CommandAction, FindAction, OverlayAction, OverlayKind, SearchAction, SearchKind,
    TestOverlayAction,
};
mod scratch;
pub use scratch::ScratchAction;
mod text_box;
pub use text_box::TextBoxAction;

use crate::input;

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
    use std::path::Path;

    use super::*;
    use crate::{
        input::Mode,
        text_box::{Direction, Motion},
    };

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
