use std::mem;

use super::{CommandLine, Editor, Error, Overlay, Result, TestOverlay};
use crate::{
    input::{
        action::{Action, EditorAction, ScratchAction},
        key::Key,
    },
    output::Output,
};

/// Where keys and actions go: to the overlay while one is open, to the scratch otherwise. What the
/// target asks of the editor itself (save, quit, opening an overlay) runs here, on the state.
impl Editor {
    pub(super) fn key_press(&mut self, key: Key) {
        // A message stays until the next key, which is what the user did after reading it
        self.status_bar.clear_message();
        // An open overlay is first in line for the keys, and takes all of them
        match &mut self.overlay {
            Some(overlay) => {
                if let Err(error) = overlay.key_press(key, &mut self.requests) {
                    self.fail(&Error::Command(error));
                }
            }
            None => self.scratch.key_press(key, &mut self.requests),
        }
        // Taken out so the requests can run on the editor, then given back for their memory
        let mut requests = mem::take(&mut self.requests);
        for action in requests.drain(..) {
            // What follows an action that failed isn't run, as it may have counted on it
            if let Err(error) = self.try_perform(&action) {
                self.fail(&error);
                break;
            }
        }
        self.requests = requests;
    }

    /// Does `action` without drawing, so the next frame shows it. An action goes to the owner of
    /// its group, whatever has the keys: an open overlay doesn't take a text box action, so a
    /// typed command works on the document and not on the overlay's own text.
    ///
    /// An action that fails is said in the status bar, as with any input.
    pub fn perform(&mut self, action: &Action) {
        if let Err(error) = self.try_perform(action) {
            self.fail(&error);
        }
        self.check_invariants();
    }

    pub(super) fn try_perform(&mut self, action: &Action) -> Result<()> {
        match action {
            Action::Editor(_) | Action::Scratch(_) => self.perform_owned(action),
            Action::TextBox(action) => {
                self.scratch.perform(*action);
                Ok(())
            }
            Action::Overlay(action) => {
                if let Some(overlay) = &mut self.overlay {
                    overlay.act(*action);
                }
                Ok(())
            }
        }
    }

    /// Does an action of the editor's group or the scratch's, the two the editor runs for the
    /// target that asked.
    fn perform_owned(&mut self, action: &Action) -> Result<()> {
        match action {
            Action::Editor(action) => self.perform_editor(action),
            Action::Scratch(action) => self.perform_scratch(*action),
            // Nothing asks the editor to run the others
            Action::TextBox(_) | Action::Overlay(_) => Ok(()),
        }
    }

    /// Does an action of the scratch's group, and writes what it gives back.
    fn perform_scratch(&mut self, action: ScratchAction) -> Result<()> {
        match action {
            ScratchAction::Save => {
                let Some(path) = self.shown_path.clone() else {
                    return Err(Error::NoPath);
                };
                if let Some(rope) = self.scratch.act(action) {
                    self.outputs.push(Output::Write { path, rope });
                }
            }
        }
        Ok(())
    }

    fn perform_editor(&mut self, action: &EditorAction) -> Result<()> {
        match action {
            EditorAction::Open(path) => self.outputs.push(Output::Read { path: path.clone() }),
            EditorAction::OpenCommandLine => {
                self.overlay = Some(Overlay::Command(Box::new(CommandLine::new(&self.config)?)));
            }
            EditorAction::CloseOverlay => self.overlay = None,
            EditorAction::OpenTestOverlay => {
                self.overlay = Some(Overlay::Test(Box::new(TestOverlay::new(&self.config)?)));
            }
            EditorAction::Quit => self.outputs.push(Output::Quit),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::{
        config::Config,
        frame::Area,
        input::{
            Input, Mode,
            action::{Scope, TextBoxAction},
            key::KeyCode,
        },
    };

    #[test]
    fn a_text_box_action_reaches_the_scratch_under_an_open_overlay() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        editor.perform(&Action::Editor(EditorAction::OpenTestOverlay));

        editor.perform(&Action::TextBox(TextBoxAction::Mode(Mode::Insert)));

        assert_eq!(
            editor.mode(),
            Mode::Insert,
            "the scratch owns the action, the overlay has the keys and not the action"
        );
    }

    #[test]
    fn open_asks_the_frontend_to_read_the_file() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        let action = Action::parse(Scope::Scratch, "open src/main.rs").unwrap();

        editor.perform(&action);

        assert!(
            editor.drain_outputs().any(
                |output| matches!(output, Output::Read { path } if &*path == Path::new("src/main.rs"))
            ),
            "the path is what was typed"
        );
    }

    fn editor_with_overlay() -> Editor {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        editor.perform(&Action::Editor(EditorAction::OpenTestOverlay));
        editor
    }

    fn press(editor: &mut Editor, code: KeyCode) {
        editor.apply(Input::Key(Key::new(code)));
    }

    #[test]
    fn escape_closes_the_overlay_through_its_own_keys() {
        let mut editor = editor_with_overlay();

        press(&mut editor, KeyCode::Esc);

        assert!(editor.overlay.is_none(), "esc is bound to close_overlay");
    }

    #[test]
    fn a_key_the_overlay_doesnt_bind_stays_in_it() {
        let mut editor = editor_with_overlay();

        press(&mut editor, KeyCode::Char('i'));

        assert!(editor.overlay.is_some(), "nothing closed it");
        assert_eq!(editor.mode(), Mode::Normal, "i didn't reach the scratch");
    }

    #[test]
    fn the_overlay_follows_the_keys_of_the_config_it_opened_with() {
        let config =
            Config::parse("[keys.test.normal]\n\"<esc>\" = []\nx = \"close_overlay\"\n").unwrap();
        let mut editor = Editor::new(Area::new(80, 24), &config).unwrap();
        editor.perform(&Action::Editor(EditorAction::OpenTestOverlay));

        press(&mut editor, KeyCode::Esc);
        assert!(editor.overlay.is_some(), "esc was unbound");
        press(&mut editor, KeyCode::Char('x'));
        assert!(editor.overlay.is_none(), "x closes it now");
    }

    #[test]
    fn save_without_a_path_is_an_error() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();

        editor.perform(&Action::Scratch(ScratchAction::Save));

        assert_eq!(
            editor.message(),
            Some("the file shown has no path to save to"),
            "nothing to write to, and the user is told"
        );
        assert!(
            !editor
                .drain_outputs()
                .any(|output| matches!(output, Output::Write { .. })),
            "and nothing written"
        );
    }

    #[test]
    fn save_writes_the_shown_text_to_its_path() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        editor.apply(Input::File {
            path: Path::new("a.txt").into(),
            bytes: b"abc".as_slice().into(),
            language: None,
        });

        editor.perform(&Action::Scratch(ScratchAction::Save));

        assert!(
            editor.drain_outputs().any(|output| matches!(
                output,
                Output::Write { path, rope } if &*path == Path::new("a.txt") && rope == "abc"
            )),
            "the path of the file and its text"
        );
    }
}
