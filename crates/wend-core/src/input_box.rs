use std::num::NonZeroUsize;

use crate::{
    config::{Config, Target},
    frame::{Area, Cell, CellPosition, Grapheme, Rect},
    input::{Mode, action::Action, key::Key},
    layout::{Canvas, Element, Layout, Pane},
    number,
    text_box::{Result, TextBox},
};

/// A one-line text box after a prompt, inside a border, which is what the command line and the
/// search type in. It starts in insert mode and answers to the keys of `target`.
///
/// After every public call the text is one line, and the box has the keys exactly while it is in
/// insert mode, which is also how it starts. `check_invariants` checks the text box it holds.
///
/// The pane draws the border and blanks what is under it, so nothing here counts the rows or
/// the columns of the border. Who owns the box decides where it goes, with the layout it is
/// given, and what its keys run.
#[derive(Debug, Clone)]
pub(crate) struct InputBox {
    pane: Pane<Prompted>,
}

/// A text box after a prompt, filling what it is given.
#[derive(Debug, Clone)]
struct Prompted {
    prompt: &'static str,
    text_box: TextBox,
}

impl InputBox {
    /// The rows of an input box, which is one for the line and the border around it.
    pub(crate) const HEIGHT: u16 = 1 + 2 * Pane::<Prompted>::BORDER;

    /// Fails when the keys of `config` for `target` can't make key trees.
    pub(crate) fn new(
        layout: Layout,
        config: &Config,
        target: Target,
        prompt: &'static str,
    ) -> Result<Self> {
        let mut text_box = TextBox::new(Rect::new(0, 0, Area::new(0, 0)), config, target)?;
        text_box.limit_lines(NonZeroUsize::MIN);
        text_box.set_mode(Mode::Insert);
        Ok(Self {
            pane: Pane::new(layout, Prompted { prompt, text_box }),
        })
    }

    pub(crate) fn text_box(&self) -> &TextBox {
        &self.pane.inner().text_box
    }

    /// The mode of the box, which says whether it has the keys: it does while it is in insert
    /// mode.
    pub(crate) fn mode(&self) -> Mode {
        self.text_box().mode()
    }

    pub(crate) fn set_mode(&mut self, mode: Mode) {
        self.pane.inner_mut().text_box.set_mode(mode);
    }

    pub(crate) fn has_focus(&self) -> bool {
        self.mode() == Mode::Insert
    }

    /// A copy of what is typed.
    pub(crate) fn to_text(&self) -> String {
        self.text_box().buffer().rope().to_string()
    }

    /// Feeds `key` to the text box, see `TextBox::key_press`.
    pub(crate) fn key_press(&mut self, key: Key, requests: &mut Vec<Action>) {
        self.pane.inner_mut().text_box.key_press(key, requests);
    }

    /// Checks the rules of the text box. Debug builds only.
    pub(crate) fn check_invariants(&self) {
        self.text_box().check_invariants();
    }
}

impl Element for InputBox {
    fn layout(&self) -> Layout {
        self.pane.layout()
    }

    fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
        self.pane.render(rect, canvas);
    }
}

impl Element for Prompted {
    fn layout(&self) -> Layout {
        Layout::FILL
    }

    /// Writes the prompt, then the text box takes the rest of the row.
    fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
        let (prompt, text) = rect.split_left(number::exact(self.prompt.chars().count()));
        for (character, col) in self
            .prompt
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
