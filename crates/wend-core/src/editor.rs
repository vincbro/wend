use std::{collections::HashSet, path::PathBuf, vec::Drain};

use crate::{
    buffer::Buffer,
    frame::{Area, Frame, Rect},
    input::{Input, Mode, action::Action, key::Key},
    output::Output,
    syntax::{LanguageTag, Loader},
    view::View,
};

mod frame_pool;
use frame_pool::FramePool;
mod keymap;
use keymap::Keymap;
mod error;
mod language;
pub use error::{Error, Result};

/// The widest the view gets, wider terminals center it.
const MAX_VIEW_COLS: u16 = 100;

/// Returns where the view goes in a terminal of `area`.
fn view_rect(area: Area) -> Rect {
    let width = area.cols.min(MAX_VIEW_COLS);
    let left_col = (area.cols - width) / 2;
    Rect::new(left_col, 0, Area::new(width, area.rows))
}

/// Holds the whole editor: the terminal's area, the view and the mode, with the buffers that
/// aren't shown and the loader of the grammars they are written in.
///
/// `buffers` holds every open buffer except the one the view shows, so each open file
/// lives in exactly one place.
#[derive(Debug)]
pub struct Editor {
    area: Area,
    buffers: Vec<Buffer>,
    view: View,
    mode: Mode,
    outputs: Vec<Output>,
    keymap: Keymap,
    frame_pool: FramePool,
    is_stale: bool,
    loader: Loader,
    /// The languages whose grammar was asked for and hasn't come yet, so a buffer opened
    /// meanwhile doesn't ask again.
    requested_languages: HashSet<LanguageTag>,
}

impl Editor {
    pub fn new(area: Area) -> Result<Self> {
        Ok(Self {
            area,
            buffers: vec![],
            view: View::new(view_rect(area)),
            mode: Mode::Normal,
            outputs: vec![],
            keymap: Keymap::default(),
            frame_pool: FramePool::new(2, area),
            is_stale: false,
            loader: Loader::new()?,
            requested_languages: HashSet::new(),
        })
    }

    /// Applies `input`, then draws a frame if the screen is out of date and a spare frame is
    /// in the pool. Inputs that arrive while every frame is out share the next frame.
    pub fn apply(&mut self, input: Input) -> Result<()> {
        let changed = !matches!(input, Input::Drawn(_));
        match input {
            Input::Action(action) => self.perform(action)?,
            Input::Key(key) => self.key_press(key)?,
            Input::Resize(area) => self.resize(area),
            Input::File {
                path,
                bytes,
                language,
            } => {
                self.show_file(path, &bytes)?;
                self.set_language(language);
            }
            Input::Drawn(frame) => self.frame_pool.place(frame),
            Input::Language {
                tag,
                wasm,
                highlights,
            } => self.load_language(tag, &wasm, &highlights),
        }

        self.is_stale |= changed;
        if self.is_stale
            && let Some(mut frame) = self.frame_pool.take()
        {
            // Parsed only when a frame is drawn, so a batch of inputs costs one parse
            self.reparse();
            // Scroll to the cursor before rendering, so the frame shows where the cursor is
            frame.set_cursor(self.view.cursor_position());
            self.update_frame(&mut frame);
            self.outputs.push(Output::Frame(frame));
            self.is_stale = false;
        }
        Ok(())
    }

    fn key_press(&mut self, key: Key) -> Result<()> {
        for action in self.keymap.press(self.mode, key) {
            self.perform(action)?;
        }
        Ok(())
    }

    fn perform(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Motion(motion) => self.view.move_selection(motion, self.is_select()),
            Action::Mode(mode) => self.set_mode(mode),
            Action::Insert(c) => self.view.insert(c),
            Action::Delete => self.view.delete(),
            Action::DeleteSelection => self.view.delete_selection(),
            Action::OpenLine => self.view.new_line(),
            Action::Revert => self.view.revert(),
            Action::Save => {
                let (path, rope) = self.view.save()?;
                self.outputs.push(Output::Write { path, rope });
            }
            Action::Quit => self.outputs.push(Output::Quit),
        }
        Ok(())
    }

    pub fn drain_outputs(&mut self) -> Drain<'_, Output> {
        self.outputs.drain(..)
    }

    #[must_use]
    pub fn view(&self) -> &View {
        &self.view
    }

    pub fn view_mut(&mut self) -> &mut View {
        &mut self.view
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Shows the file at `path`, reusing its buffer if it's among the open buffers. The buffer
    /// shown before joins them.
    fn show_file(&mut self, path: PathBuf, bytes: &[u8]) -> Result<()> {
        if self.view.buffer().path() == Some(path.as_path()) {
            self.view.update(bytes)?;
        } else if let Some(index) = self
            .buffers
            .iter()
            .position(|buffer| buffer.path() == Some(path.as_path()))
        {
            let mut buffer = self.buffers.swap_remove(index);
            buffer.update(bytes)?;
            self.buffers.push(self.view.open_buffer(buffer));
        } else {
            self.buffers
                .push(self.view.open_buffer(Buffer::from_bytes(path, bytes)?));
        }
        Ok(())
    }

    /// Builds a new frame of the terminal's size, with the cursor where the head is now.
    pub fn frame(&mut self) -> Frame {
        // Scroll to the cursor before rendering, so the frame shows where the cursor is
        let cursor = self.view.cursor_position();
        let mut frame = Frame::new(self.area, cursor);
        self.update_frame(&mut frame);
        frame
    }

    /// Redraws the view into `frame`, reusing its cells. `Frame::cursor` stays as it was, only
    /// `frame` gives a new one.
    ///
    /// # Panics
    ///
    /// If `frame` is smaller than the view reaches, as one from before a resize can be.
    pub fn update_frame(&mut self, frame: &mut Frame) {
        self.view.render(frame);
    }

    /// Fits the editor to a new terminal size, keeping the view capped and centered.
    pub fn resize(&mut self, area: Area) {
        self.area = area;
        self.view.resize(view_rect(area));
        self.frame_pool.resize(area);
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    #[must_use]
    pub fn is_select(&self) -> bool {
        matches!(self.mode, Mode::Select)
    }

    #[must_use]
    pub fn is_insert(&self) -> bool {
        matches!(self.mode, Mode::Insert)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{input::key::KeyCode, syntax};

    fn press(editor: &mut Editor, keys: &str) {
        for c in keys.chars() {
            let code = match c {
                '\u{1b}' => KeyCode::Esc,
                '\n' => KeyCode::Enter,
                '\u{8}' => KeyCode::Backspace,
                c => KeyCode::Char(c),
            };
            editor.apply(Input::Key(Key::new(code))).unwrap();
        }
    }

    fn text(editor: &Editor) -> String {
        editor.view().buffer().rope().to_string()
    }

    #[test]
    fn insert_types_and_esc_leaves() {
        let mut editor = Editor::new(Area::new(80, 24)).unwrap();
        press(&mut editor, "iab\u{8}c\u{1b}");
        assert_eq!(text(&editor), "ac", "backspace deletes the b");
        assert_eq!(editor.mode(), Mode::Normal);
    }

    #[test]
    fn o_opens_a_line_and_inserts() {
        let mut editor = Editor::new(Area::new(80, 24)).unwrap();
        press(&mut editor, "ia\u{1b}ob");
        assert_eq!(text(&editor), "a\nb");
        assert_eq!(editor.mode(), Mode::Insert);
    }

    #[test]
    fn v_toggles_select() {
        let mut editor = Editor::new(Area::new(80, 24)).unwrap();
        press(&mut editor, "v");
        assert_eq!(editor.mode(), Mode::Select);
        press(&mut editor, "v");
        assert_eq!(editor.mode(), Mode::Normal);
    }

    #[test]
    fn select_then_delete_removes_the_selection() {
        let mut editor = Editor::new(Area::new(80, 24)).unwrap();
        press(&mut editor, "iab cd\u{1b}gsvwd");
        assert_eq!(
            text(&editor),
            "cd",
            "w selects the first word and the space after it"
        );
    }

    #[test]
    fn revert_drops_unsaved_edits() {
        let mut editor = Editor::new(Area::new(80, 24)).unwrap();
        press(&mut editor, "iab\u{1b}u");
        assert_eq!(text(&editor), "");
    }

    #[test]
    fn q_asks_to_quit() {
        let mut editor = Editor::new(Area::new(80, 24)).unwrap();
        press(&mut editor, "q");
        assert!(
            editor
                .drain_outputs()
                .any(|output| matches!(output, Output::Quit))
        );
    }

    #[test]
    fn a_file_opens_and_then_updates() {
        let mut editor = Editor::new(Area::new(80, 24)).unwrap();
        let path = PathBuf::from("a.txt");
        let file = |bytes: &[u8]| Input::File {
            path: path.clone(),
            bytes: bytes.into(),
            language: None,
        };

        editor.apply(file(b"abc")).unwrap();
        assert_eq!(text(&editor), "abc");
        assert_eq!(editor.view().buffer().path(), Some(path.as_path()));

        editor.apply(file(b"xyz")).unwrap();
        assert_eq!(
            text(&editor),
            "xyz",
            "the same path updates the shown buffer"
        );
        assert!(
            editor.buffers.iter().all(|buffer| buffer.path().is_none()),
            "only the empty buffer the editor started with, no second copy of the file"
        );
    }

    /// Takes the outputs and returns the frames among them.
    fn frames(editor: &mut Editor) -> Vec<Frame> {
        editor
            .drain_outputs()
            .filter_map(|output| match output {
                Output::Frame(frame) => Some(frame),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_drawn_frame_alone_draws_nothing() {
        let mut editor = Editor::new(Area::new(80, 24)).unwrap();
        press(&mut editor, "i");
        let mut sent = frames(&mut editor);
        assert_eq!(sent.len(), 1);

        editor.apply(Input::Drawn(sent.remove(0))).unwrap();
        assert!(
            frames(&mut editor).is_empty(),
            "nothing changed, so no redraw loop"
        );
    }

    #[test]
    fn inputs_while_every_frame_is_out_share_one_frame() {
        let mut editor = Editor::new(Area::new(80, 24)).unwrap();
        press(&mut editor, "i");
        press(&mut editor, "a");
        let drawing = frames(&mut editor);
        assert_eq!(drawing.len(), 2, "both frames are out");

        press(&mut editor, "bcd");
        assert!(
            frames(&mut editor).is_empty(),
            "no spare frame to draw into"
        );

        for frame in drawing {
            editor.apply(Input::Drawn(frame)).unwrap();
        }
        let sent = frames(&mut editor);
        assert_eq!(sent.len(), 1, "the three keys show up in one frame");
        assert!(
            sent[0]
                .iter()
                .any(|(_, cell)| cell.grapheme.as_str() == "d"),
            "the frame has the latest text"
        );
    }

    #[test]
    fn a_loaded_grammar_reaches_the_buffers_that_are_not_shown() {
        let mut editor = Editor::new(Area::new(80, 24)).unwrap();
        let text = "fn a() {}";
        let mut hidden = Buffer::from(text);
        hidden.set_language(Some(LanguageTag::new("rust")));
        editor.buffers.push(hidden);

        editor.load_language(
            LanguageTag::new("rust"),
            syntax::tests::RUST_WASM,
            syntax::tests::RUST_HIGHLIGHTS,
        );
        editor.buffers[0].reparse(&mut editor.loader).unwrap();

        assert_ne!(
            editor.buffers[0].highlights(0..text.len()).count(),
            0,
            "the buffer got the grammar while it was out of sight"
        );
    }
}
