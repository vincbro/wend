use std::num::NonZeroUsize;

use crate::{
    config::{self, Config, Target},
    frame::{Area, Cell, CellPosition, Grapheme, Rect},
    input::{
        Mode,
        action::{Action, CommandAction, EditorAction, OverlayAction, OverlayKind, Scope},
        key::Key,
    },
    layout::{Canvas, Element, Fit, Layout, Pane, Size},
    number,
    text_box::{Result, TextBox},
};

/// What stands before the text of the command.
const PROMPT: &str = ": ";

/// How wide the box the command is typed in is, or all of the editor when it is narrower, so
/// that a small terminal still has room for the text.
const WIDTH: Size = Size::Percent(50);

/// The one-line text box with the keys of `[keys.command.<mode>]`, in a bordered pane over the
/// middle of the editor. Running it closes it and runs the action its text names in the
/// scratch's scope.
///
/// The pane draws the border and blanks what is under it, so nothing here counts rows or columns
/// of the border.
#[derive(Debug)]
pub(in crate::editor) struct CommandLine {
    pane: Pane<Prompted>,
}

/// A text box after a prompt, filling what it is given.
#[derive(Debug)]
struct Prompted {
    text_box: TextBox,
}

impl CommandLine {
    pub(in crate::editor) fn new(config: &Config) -> Result<Self> {
        let mut text_box = TextBox::new(
            Rect::new(0, 0, Area::new(0, 0)),
            config,
            Target::Overlay(OverlayKind::Command),
        )?;
        text_box.limit_lines(NonZeroUsize::MIN);
        text_box.set_mode(Mode::Insert);
        Ok(Self {
            pane: Pane::new(Layout::FILL, Prompted { text_box }),
        })
    }

    fn text_box(&self) -> &TextBox {
        &self.pane.inner().text_box
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
        self.pane.inner_mut().text_box.key_press(key, requests);
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
        let text = self.text_box().buffer().rope().to_string();
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        requests.push(Action::parse(Scope::Scratch, text)?);
        Ok(())
    }

    /// Checks the rules of the text box. Debug builds only.
    pub(in crate::editor) fn check_invariants(&self) {
        self.text_box().check_invariants();
    }
}

impl Element for CommandLine {
    /// The pane at the middle of the editor, whose height is one line inside its border.
    fn layout(&self) -> Layout {
        Layout::Float(Fit::centered(
            WIDTH,
            Size::Fixed(1 + 2 * Pane::<Prompted>::BORDER),
        ))
    }

    fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
        canvas.child(&mut self.pane, rect);
    }
}

impl Element for Prompted {
    fn layout(&self) -> Layout {
        Layout::FILL
    }

    /// Writes the prompt, then the text box takes the rest of the row.
    fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
        let (prompt, text) = rect.split_left(number::exact(PROMPT.chars().count()));
        for (character, col) in PROMPT
            .chars()
            .zip(prompt.col..prompt.col + prompt.area.cols)
        {
            let cell = Cell {
                grapheme: Grapheme::from_char(character),
                ..Cell::default()
            };
            canvas.write(CellPosition::new(col, prompt.row), cell);
        }
        canvas.child(&mut self.text_box, text);
    }
}
