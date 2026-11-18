use std::{collections::HashSet, mem, path::Path, vec::Drain};

use crate::{
    buffer::Buffer,
    config::Config,
    frame::{Area, Frame, Rect},
    input::{
        Input, Mode,
        action::{Action, EditorAction},
        key::Key,
    },
    layout::{Canvas, Element},
    output::Output,
    syntax::{LanguageTag, Loader},
    view::View,
};

mod frame_pool;
use frame_pool::FramePool;
mod status_bar;
use status_bar::StatusBar;
mod overlay;
use overlay::{KeyOutcome, Overlay, TestOverlay};
mod error;
mod language;
pub use error::{Error, Result};

/// How many frames pass between the editor and the frontend. The frontend keeps the one on
/// screen to draw only what differs from it, so one is always away, and the others are the
/// editor's to draw into.
const FRAME_COUNT: usize = 3;

/// Holds the whole editor: the terminal's area, the view and the overlay over it, with the
/// buffers that aren't shown and the loader of the grammars they are written in.
///
/// `buffers` holds every open buffer except the one the view shows, so each open file
/// lives in exactly one place.
#[derive(Debug)]
pub struct Editor {
    area: Area,
    buffers: Vec<Buffer>,
    view: View,
    status_bar: StatusBar,
    /// The window floating over the editor, which gets the keys while it is open.
    overlay: Option<Overlay>,
    /// What the editor follows, kept for the views of the windows it opens.
    config: Config,
    outputs: Vec<Output>,
    /// What the view asked of the editor, kept so that its memory is reused.
    requests: Vec<EditorAction>,
    frame_pool: FramePool,
    is_stale: bool,
    loader: Loader,
    /// The languages whose grammar was asked for and hasn't come yet, so a buffer opened
    /// meanwhile doesn't ask again.
    requested_languages: HashSet<LanguageTag>,
}

impl Editor {
    /// Starts an editor that follows `config`.
    pub fn new(area: Area, config: &Config) -> Result<Self> {
        let view = View::new(Rect::new(0, 0, area), config)?;
        let mut editor = Self {
            area,
            buffers: vec![],
            view,
            status_bar: StatusBar,
            overlay: None,
            config: config.clone(),
            outputs: vec![],
            requests: vec![],
            frame_pool: FramePool::new(FRAME_COUNT, area),
            is_stale: false,
            loader: Loader::new()?,
            requested_languages: HashSet::new(),
        };
        editor.fit_view();
        Ok(editor)
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
            Input::Config(config) => self.set_config(&config)?,
        }

        self.is_stale |= changed;
        if self.is_stale
            && let Some(mut frame) = self.frame_pool.take()
        {
            // Parsed only when a frame is drawn, so a batch of inputs costs one parse
            self.reparse();
            self.update_frame(&mut frame);
            self.outputs.push(Output::Frame(frame));
            self.is_stale = false;
        }
        Ok(())
    }

    /// Follows `config` from now on. A config that fails leaves the editor as it was.
    pub fn set_config(&mut self, config: &Config) -> Result<()> {
        self.view.set_config(config)?;
        self.config.clone_from(config);
        Ok(())
    }

    fn key_press(&mut self, key: Key) -> Result<()> {
        // An open overlay is first in line for the keys, and takes all of them
        if let Some(overlay) = &mut self.overlay {
            if overlay.key(key) == KeyOutcome::Closed {
                self.overlay = None;
            }
            return Ok(());
        }
        self.view.key_press(key, &mut self.requests);
        // Taken out so the requests can run on the editor, then given back for their memory
        let mut requests = mem::take(&mut self.requests);
        let result = requests
            .drain(..)
            .try_for_each(|action| self.perform_editor(action));
        self.requests = requests;
        result
    }

    fn perform(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Editor(action) => self.perform_editor(action),
            Action::View(action) => {
                self.view.perform(action);
                Ok(())
            }
        }
    }

    fn perform_editor(&mut self, action: EditorAction) -> Result<()> {
        match action {
            EditorAction::Save => {
                let (path, rope) = self.view.save()?;
                self.outputs.push(Output::Write { path, rope });
            }
            EditorAction::OpenTestOverlay => {
                self.overlay = Some(Overlay::Test(TestOverlay::new(&self.config)?));
            }
            EditorAction::Quit => self.outputs.push(Output::Quit),
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
        self.view.mode()
    }

    /// Shows the file at `path`, reusing its buffer if it's among the open buffers. The buffer
    /// shown before joins them.
    fn show_file(&mut self, path: Box<Path>, bytes: &[u8]) -> Result<()> {
        if self.view.buffer().path() == Some(&*path) {
            self.view.update(bytes)?;
        } else if let Some(index) = self
            .buffers
            .iter()
            .position(|buffer| buffer.path() == Some(&*path))
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
        let mut frame = Frame::new(self.area, None);
        self.update_frame(&mut frame);
        frame
    }

    /// Draws the whole editor into `frame`, reusing its cells. What it shows from before is
    /// blanked first.
    pub fn update_frame(&mut self, frame: &mut Frame) {
        let bounds = Rect::new(0, 0, self.area);
        let mut canvas = Canvas::root(frame);
        // The status bar docks first, so the view gets the rows that are left
        let free = canvas.child(&mut self.status_bar, bounds);
        canvas.child(&mut self.view, free);
        if let Some(overlay) = &mut self.overlay {
            // A float is placed in the whole frame, so it doesn't matter what is free
            canvas.child(overlay, bounds);
        }
    }

    /// Fits the editor to a new terminal size.
    pub fn resize(&mut self, area: Area) {
        self.area = area;
        self.fit_view();
        self.frame_pool.resize(area);
    }

    /// Gives the view the rect its layout works out to, the same way a draw does, so the keys
    /// that come before the next draw already act on it.
    fn fit_view(&mut self) {
        let bounds = Rect::new(0, 0, self.area);
        let (_, free) = self.status_bar.layout().place(bounds, bounds);
        let (rect, _) = self.view.layout().place(free, bounds);
        self.view.resize(rect);
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.view.set_mode(mode);
    }

    #[must_use]
    pub fn is_select(&self) -> bool {
        matches!(self.view.mode(), Mode::Select)
    }

    #[must_use]
    pub fn is_insert(&self) -> bool {
        matches!(self.view.mode(), Mode::Insert)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Actions,
        input::{action::ViewAction, key::KeyCode},
        syntax,
    };

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
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        press(&mut editor, "iab\u{8}c\u{1b}");
        assert_eq!(text(&editor), "ac", "backspace deletes the b");
        assert_eq!(editor.mode(), Mode::Normal);
    }

    #[test]
    fn o_opens_a_line_and_inserts() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        press(&mut editor, "ia\u{1b}ob");
        assert_eq!(text(&editor), "a\nb");
        assert_eq!(editor.mode(), Mode::Insert);
    }

    #[test]
    fn a_new_config_rebinds_keys() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        let mut config = Config::default();
        config.keys.normal.insert(
            "<leader>v".parse().unwrap(),
            Actions(Box::new([Action::View(ViewAction::Mode(Mode::Select))])),
        );
        editor.apply(Input::Config(Box::new(config))).unwrap();
        press(&mut editor, " v");
        assert_eq!(editor.mode(), Mode::Select, "space is the leader");
    }

    #[test]
    fn a_config_that_collides_leaves_the_keys_as_they_were() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        let mut config = Config::default();
        config.keys.normal.insert(
            "g".parse().unwrap(),
            Actions(Box::new([Action::Editor(EditorAction::Quit)])),
        );
        assert!(editor.set_config(&config).is_err(), "g also starts gs");
        press(&mut editor, "gs");
        assert!(
            !editor
                .drain_outputs()
                .any(|output| matches!(output, Output::Quit)),
            "g stayed a prefix"
        );
    }

    #[test]
    fn esc_collapses_the_selection_and_leaves_select() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        press(&mut editor, "iab cd\u{1b}gsvw\u{1b}d");
        assert_eq!(
            text(&editor),
            "abcd",
            "only the grapheme under the cursor went, not the whole word"
        );
        assert_eq!(editor.mode(), Mode::Normal);
    }

    #[test]
    fn changing_mode_keeps_the_selection() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        press(&mut editor, "iab cd\u{1b}gsvwvd");
        assert_eq!(text(&editor), "cd", "v back to normal didn't drop the word");
    }

    #[test]
    fn v_toggles_select() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        press(&mut editor, "v");
        assert_eq!(editor.mode(), Mode::Select);
        press(&mut editor, "v");
        assert_eq!(editor.mode(), Mode::Normal);
    }

    #[test]
    fn select_then_delete_removes_the_selection() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        press(&mut editor, "iab cd\u{1b}gsvwd");
        assert_eq!(
            text(&editor),
            "cd",
            "w selects the first word and the space after it"
        );
    }

    #[test]
    fn x_selects_the_row_the_cursor_is_on() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        press(&mut editor, "iab\ncd\nef\u{1b}kxd");
        assert_eq!(
            text(&editor),
            "ab\nef",
            "x took the middle row and nothing else"
        );
    }

    #[test]
    fn x_grows_a_selection_over_the_next_row() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        press(&mut editor, "iab\ncd\nef\u{1b}kxvjxd");
        assert_eq!(
            text(&editor),
            "ab\n",
            "x in select mode joined the next row to the selection"
        );
    }

    #[test]
    fn revert_drops_unsaved_edits() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        press(&mut editor, "iab\u{1b}u");
        assert_eq!(text(&editor), "");
    }

    #[test]
    fn q_asks_to_quit() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        press(&mut editor, "q");
        assert!(
            editor
                .drain_outputs()
                .any(|output| matches!(output, Output::Quit))
        );
    }

    #[test]
    fn a_file_opens_and_then_updates() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        let path: Box<Path> = Path::new("a.txt").into();
        let file = |bytes: &[u8]| Input::File {
            path: path.clone(),
            bytes: bytes.into(),
            language: None,
        };

        editor.apply(file(b"abc")).unwrap();
        assert_eq!(text(&editor), "abc");
        assert_eq!(editor.view().buffer().path(), Some(&*path));

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
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
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
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        // One key for each frame, the first of them entering insert mode
        press(&mut editor, "i");
        press(&mut editor, &"a".repeat(FRAME_COUNT - 1));
        let drawing = frames(&mut editor);
        assert_eq!(drawing.len(), FRAME_COUNT, "every frame is out");

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
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
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
