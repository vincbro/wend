use crate::{
    config::{self, Config, Target},
    frame::Rect,
    input::{
        action::{Action, CommandAction, EditorAction, OverlayAction, OverlayKind, Scope},
        key::Key,
    },
    input_box::InputBox,
    layout::{Canvas, Element, Fit, Layout, Size},
    text_box::Result,
};

/// What stands before the text of the command.
const PROMPT: &str = ": ";

/// How wide the box the command is typed in is, or all of the editor when it is narrower, so
/// that a small terminal still has room for the text.
const WIDTH: Size = Size::Percent(50);

/// An input box with the keys of `[keys.command.<mode>]`, over the middle of the editor. Running
/// it closes it and runs the action its text names in the scratch's scope.
#[derive(Debug)]
pub(in crate::editor) struct CommandLine {
    input: InputBox,
}

impl CommandLine {
    pub(in crate::editor) fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            input: InputBox::new(
                Layout::FILL,
                config,
                Target::Overlay(OverlayKind::Command),
                PROMPT,
            )?,
        })
    }

    /// Feeds `key` to the text box and, when it ran the command, replaces what it asked for with
    /// closing the command line and the action the text names. The others are added to
    /// `requests`, which this clears first.
    ///
    /// Fails when the text names no action, after asking for the command line to close.
    pub(in crate::editor) fn key_press(
        &mut self,
        key: Key,
        requests: &mut Vec<Action>,
    ) -> config::Result<()> {
        self.input.key_press(key, requests);
        let runs = requests.iter().any(|action| {
            matches!(
                action,
                Action::Overlay(OverlayAction::Command(CommandAction::Run))
            )
        });
        if !runs {
            return Ok(());
        }
        requests.clear();
        requests.push(Action::Editor(EditorAction::CloseOverlay));
        let text = self.input.to_text();
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        requests.push(Action::parse(Scope::Scratch, text)?);
        Ok(())
    }

    /// Checks the rules of the text box. Debug builds only.
    pub(in crate::editor) fn check_invariants(&self) {
        self.input.check_invariants();
    }
}

impl Element for CommandLine {
    /// The input box at the middle of the editor.
    fn layout(&self) -> Layout {
        Layout::Float(Fit::centered(WIDTH, Size::Fixed(InputBox::HEIGHT)))
    }

    fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
        canvas.child(&mut self.input, rect);
    }
}
