use std::{collections::HashSet, iter, path::Path, vec::Drain};

use crate::{
    buffer::Buffer,
    config::Config,
    frame::{Area, Frame, Rect},
    input::{Failure, Input, Mode, action::Action},
    layout::{Canvas, Element},
    output::Output,
    scratch::Scratch,
    syntax::{LanguageTag, Loader},
};

mod frame_pool;
use frame_pool::FramePool;
mod status_bar;
use status_bar::StatusBar;
mod overlay;
use overlay::{CommandLine, Overlay, TestOverlay};
mod document;
use document::Document;
mod error;
mod language;
mod route;
pub use error::{Error, Result};

/// How many frames pass between the editor and the frontend. The frontend keeps the one on
/// screen to draw only what differs from it, so one is always away, and the others are the
/// editor's to draw into.
const FRAME_COUNT: usize = 3;

/// Holds the whole editor: the terminal's area, the scratch and the overlay over it, with the
/// documents that aren't shown and the loader of the grammars they are written in.
///
/// `documents` holds every open document except the one the scratch shows, whose path is
/// `shown_path`, so each open file lives in exactly one place.
///
/// After every public call the scratch fills what the status bar leaves of the terminal, every
/// buffer and the scratch keep their own invariants, a language that is being waited for isn't
/// loaded yet, and the editor is only out of date when every frame is out being drawn.
/// `check_invariants` checks them.
#[derive(Debug)]
pub struct Editor {
    area: Area,
    documents: Vec<Document>,
    /// The file the scratch's buffer came from, none for a buffer that never had one.
    shown_path: Option<Box<Path>>,
    scratch: Scratch,
    status_bar: StatusBar,
    /// The window floating over the editor, which gets the keys while it is open.
    overlay: Option<Overlay>,
    /// What the editor follows, kept for the text boxes of the windows it opens.
    config: Config,
    outputs: Vec<Output>,
    /// What the scratch asked of the editor, kept so that its memory is reused.
    requests: Vec<Action>,
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
        Self::with_loader(area, config, Loader::new()?)
    }

    /// Starts an editor that follows `config` and loads grammars with `loader`.
    fn with_loader(area: Area, config: &Config, loader: Loader) -> Result<Self> {
        let scratch = Scratch::new(Rect::new(0, 0, area), config)?;
        let mut editor = Self {
            area,
            documents: vec![],
            shown_path: None,
            scratch,
            status_bar: StatusBar::new(),
            overlay: None,
            config: config.clone(),
            outputs: vec![],
            requests: vec![],
            frame_pool: FramePool::new(FRAME_COUNT, area),
            is_stale: false,
            loader,
            requested_languages: HashSet::new(),
        };
        editor.fit_scratch();
        editor.check_invariants();
        Ok(editor)
    }

    /// Applies `input`, then draws a frame if the screen is out of date and a spare frame is
    /// in the pool. Inputs that arrive while every frame is out share the next frame.
    ///
    /// An input that fails is not an error of this call: the editor says so in the status bar
    /// and goes on, so a frontend can't end the editor with one.
    pub fn apply(&mut self, input: Input) {
        let changed = !matches!(input, Input::Drawn(_));
        if let Err(error) = self.try_apply(input) {
            self.fail(&error);
        }

        self.is_stale |= changed;
        if self.is_stale
            && let Some(mut frame) = self.frame_pool.take()
        {
            // Cleared first, so the editor is never out of date with a frame in hand
            self.is_stale = false;
            // Parsed only when a frame is drawn, so a batch of inputs costs one parse
            self.reparse();
            self.update_frame(&mut frame);
            self.outputs.push(Output::Frame(frame));
        }
        // Checked whether or not the input worked, a failure leaves the editor consistent too
        self.check_invariants();
    }

    fn try_apply(&mut self, input: Input) -> Result<()> {
        match input {
            Input::Action(action) => self.try_perform(&action)?,
            Input::Key(key) => self.key_press(key),
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
            Input::Config(config) => self.try_set_config(&config)?,
            Input::Failed(failure) => self.fail(&Error::Frontend(failure)),
        }
        Ok(())
    }

    /// Says `error` in the status bar and undoes what it leaves half done. It is the only place
    /// an error becomes what the user sees, so every failure that isn't a bug ends up here.
    pub(super) fn fail(&mut self, error: &Error) {
        // The grammar isn't coming, so a buffer opened later may ask for it again
        if let Error::Frontend(Failure::Language { tag, .. }) = &error {
            self.requested_languages.remove(tag);
        }
        self.status_bar.show(&error.describe());
    }

    /// What the status bar says about a failure, if it says anything.
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.status_bar.message()
    }

    /// Follows `config` from now on. A config that fails leaves the editor as it was, and says
    /// so in the status bar.
    pub fn set_config(&mut self, config: &Config) {
        if let Err(error) = self.try_set_config(config) {
            self.fail(&error);
        }
        self.check_invariants();
    }

    fn try_set_config(&mut self, config: &Config) -> Result<()> {
        self.scratch.set_config(config)?;
        self.config.clone_from(config);
        Ok(())
    }

    pub fn drain_outputs(&mut self) -> Drain<'_, Output> {
        self.outputs.drain(..)
    }

    #[must_use]
    pub fn scratch(&self) -> &Scratch {
        &self.scratch
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.scratch.mode()
    }

    /// Shows the file at `path`, reusing its document if it's among the open documents. The
    /// document shown before joins them.
    fn show_file(&mut self, path: Box<Path>, bytes: &[u8]) -> Result<()> {
        if self.shown_path.as_deref() == Some(&*path) {
            self.scratch.update(bytes)?;
            return Ok(());
        }
        let buffer = match self
            .documents
            .iter()
            .position(|document| document.path() == Some(&*path))
        {
            Some(index) => {
                let (_, mut buffer) = self.documents.swap_remove(index).into_parts();
                buffer.update(bytes)?;
                buffer
            }
            None => Buffer::from_bytes(bytes)?,
        };
        let hidden = self.scratch.open_buffer(buffer);
        let hidden_path = self.shown_path.replace(path);
        self.documents.push(Document::new(hidden_path, hidden));
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
        // The status bar docks first, so the scratch gets the rows that are left
        let mode = match self.scratch.mode() {
            Mode::Normal => "nor",
            Mode::Select => "sel",
            Mode::Insert => "ins",
        };
        self.status_bar.update(mode);
        let free = canvas.child(&mut self.status_bar, bounds);
        canvas.child(&mut self.scratch, free);
        if let Some(overlay) = &mut self.overlay {
            // A float is placed in the whole frame, so it doesn't matter what is free
            canvas.child(overlay, bounds);
        }
        self.check_invariants();
    }

    /// Fits the editor to a new terminal size.
    pub fn resize(&mut self, area: Area) {
        self.area = area;
        self.fit_scratch();
        self.frame_pool.resize(area);
        self.check_invariants();
    }

    /// Gives the scratch the rect its layout works out to, the same way a draw does, so the keys
    /// that come before the next draw already act on it.
    fn fit_scratch(&mut self) {
        self.scratch.resize(self.scratch_rect());
    }

    /// Returns the rect the scratch fills, what the status bar leaves of the terminal.
    fn scratch_rect(&self) -> Rect {
        let bounds = Rect::new(0, 0, self.area);
        let (_, free) = self.status_bar.layout().place(bounds, bounds);
        let (rect, _) = self.scratch.layout().place(free, bounds);
        rect
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.scratch.set_mode(mode);
    }

    #[must_use]
    pub fn is_select(&self) -> bool {
        matches!(self.scratch.mode(), Mode::Select)
    }

    #[must_use]
    pub fn is_insert(&self) -> bool {
        matches!(self.scratch.mode(), Mode::Insert)
    }
}

impl Editor {
    /// Checks the rules every public function has to leave the editor in, and the ones of what
    /// it holds. Debug builds only.
    fn check_invariants(&self) {
        if !cfg!(debug_assertions) {
            return;
        }

        self.scratch.check_invariants();
        if let Some(overlay) = &self.overlay {
            overlay.check_invariants();
        }
        for document in &self.documents {
            document.buffer().check_invariants();
        }
        let paths: Vec<_> = iter::once(self.shown_path.as_deref())
            .chain(self.documents.iter().map(Document::path))
            .flatten()
            .collect();
        for (index, path) in paths.iter().enumerate() {
            assert!(
                !paths[..index].contains(path),
                "show_file reuses the buffer of a file that is open, so {} is in one place",
                path.display()
            );
        }

        assert_eq!(
            self.scratch.rect(),
            self.scratch_rect(),
            "new and resize fit the scratch to what the status bar leaves"
        );
        assert!(
            self.requested_languages
                .iter()
                .all(|tag| self.loader.grammar(tag).is_none()),
            "a language is only asked for while its grammar isn't loaded, and load_language \
             stops waiting for it"
        );
        assert!(
            !self.is_stale || self.frame_pool.is_empty(),
            "apply draws a stale editor as soon as a frame is spare"
        );
    }
}

#[cfg(test)]
mod fuzz;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Actions,
        input::{
            action::{Action, EditorAction, TextBoxAction},
            key::{Key, KeyCode},
        },
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
            editor.apply(Input::Key(Key::new(code)));
        }
    }

    fn text(editor: &Editor) -> String {
        editor.scratch().buffer().rope().to_string()
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
            Actions(Box::new([Action::TextBox(TextBoxAction::Mode(
                Mode::Select,
            ))])),
        );
        editor.apply(Input::Config(Box::new(config)));
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
        editor.set_config(&config);
        let message = editor.message().expect("g also starts gs, and it is said");
        assert!(
            message.ends_with("is bound as a key and also as the start of a longer combo"),
            "the message has the cause and not only what was being done: {message}"
        );
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

    fn quit_was_asked(editor: &mut Editor) -> bool {
        editor
            .drain_outputs()
            .any(|output| matches!(output, Output::Quit))
    }

    #[test]
    fn the_command_line_quits() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        press(&mut editor, ":q\n");
        assert!(quit_was_asked(&mut editor));
    }

    #[test]
    fn q_and_s_are_unbound_by_default_and_a_config_can_bind_them() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        press(&mut editor, "qs");
        assert!(!quit_was_asked(&mut editor), "q quits nothing by default");

        let config = Config::parse("[keys.scratch.normal]\nq = \"quit\"\n").unwrap();
        let mut editor = Editor::new(Area::new(80, 24), &config).unwrap();
        press(&mut editor, "q");
        assert!(quit_was_asked(&mut editor), "a config binds it");
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

        editor.apply(file(b"abc"));
        assert_eq!(text(&editor), "abc");
        assert_eq!(editor.shown_path.as_deref(), Some(&*path));

        editor.apply(file(b"xyz"));
        assert_eq!(
            text(&editor),
            "xyz",
            "the same path updates the shown buffer"
        );
        assert!(
            editor
                .documents
                .iter()
                .all(|document| document.path().is_none()),
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

        editor.apply(Input::Drawn(sent.remove(0)));
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
            editor.apply(Input::Drawn(frame));
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
        editor.documents.push(Document::new(None, hidden));

        editor.load_language(
            LanguageTag::new("rust"),
            syntax::tests::RUST_WASM,
            syntax::tests::RUST_HIGHLIGHTS,
        );
        editor.documents[0]
            .buffer_mut()
            .reparse(&mut editor.loader)
            .unwrap();

        assert_ne!(
            editor.documents[0]
                .buffer_mut()
                .highlights(0..text.len())
                .count(),
            0,
            "the buffer got the grammar while it was out of sight"
        );
    }
}
