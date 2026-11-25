use std::mem;

use super::{Editor, Error, Result};
use crate::{
    input::{
        action::{Action, EditorAction, ScratchAction},
        key::Key,
    },
    output::Output,
    overlay::{CommandLine, Overlay, SearchOverlay, TestOverlay},
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
                let result = overlay.key_press(key, &mut self.requests);
                self.outputs.extend(overlay.take_ask());
                if let Err(error) = result {
                    self.fail(&Error::Command(error));
                }
            }
            None => self.scratch.key_press(key, &mut self.requests),
        }
        self.run_requests();
    }

    /// Runs the actions in `requests`, which a key or an answer from the frontend left there.
    pub(super) fn run_requests(&mut self) {
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
            Action::Scratch(action) => self.perform_scratch(action),
            // Nothing asks the editor to run the others
            Action::TextBox(_) | Action::Overlay(_) => Ok(()),
        }
    }

    /// Does an action of the scratch's group, and writes what it gives back.
    ///
    /// A save with a path writes there. For an unnamed document that names it, so it is clean
    /// and later saves go there. For a named one it is a copy: the document keeps its path and
    /// stays as dirty as it was.
    fn perform_scratch(&mut self, action: &ScratchAction) -> Result<()> {
        match action {
            ScratchAction::Save(target) => {
                let plan = self.workspace.save_plan(target.as_deref())?;
                let Some(rope) = self.scratch.act(action, plan.is_copy) else {
                    return Ok(());
                };
                if plan.names {
                    self.workspace.name_shown(plan.path.clone());
                }
                self.outputs.push(Output::Write {
                    path: plan.path,
                    rope,
                });
            }
        }
        Ok(())
    }

    fn perform_editor(&mut self, action: &EditorAction) -> Result<()> {
        match action {
            EditorAction::Open(path) => self.outputs.push(Output::Read { path: path.clone() }),
            EditorAction::OpenAt { path, line } => {
                self.workspace.show_at(path.clone(), *line);
                self.outputs.push(Output::Read { path: path.clone() });
            }
            EditorAction::OpenCommandLine => {
                self.overlay = Some(Overlay::Command(Box::new(CommandLine::new(&self.config)?)));
            }
            EditorAction::OpenSearch(kind) => {
                let mut search = SearchOverlay::new(&self.config, *kind)?;
                self.outputs.extend(search.take_ask());
                self.overlay = Some(Overlay::Search(Box::new(search)));
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

        editor.perform(&Action::Scratch(ScratchAction::Save(None)));

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

        editor.perform(&Action::Scratch(ScratchAction::Save(None)));

        assert!(
            editor.drain_outputs().any(|output| matches!(
                output,
                Output::Write { path, rope } if &*path == Path::new("a.txt") && rope == "abc"
            )),
            "the path of the file and its text"
        );
    }

    fn written(editor: &mut Editor) -> Vec<(Box<Path>, String)> {
        editor
            .drain_outputs()
            .filter_map(|output| match output {
                Output::Write { path, rope } => Some((path, rope.to_string())),
                _ => None,
            })
            .collect()
    }

    fn save_to(path: &str) -> Action {
        Action::parse(Scope::Scratch, &format!("write {path}")).unwrap()
    }

    #[test]
    fn writing_an_unnamed_document_to_a_path_names_it() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        editor.perform(&Action::TextBox(TextBoxAction::Mode(Mode::Insert)));
        editor.perform(&Action::TextBox(TextBoxAction::Insert('x')));

        editor.perform(&save_to("new.txt"));
        editor.perform(&Action::Scratch(ScratchAction::Save(None)));

        let writes = written(&mut editor);
        assert_eq!(
            writes,
            vec![
                (Path::new("new.txt").into(), "x".to_owned()),
                (Path::new("new.txt").into(), "x".to_owned())
            ],
            "the second save goes to the name it was given"
        );
    }

    #[test]
    fn writing_a_named_document_to_another_path_is_a_copy() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        editor.apply(Input::File {
            path: Path::new("a.txt").into(),
            bytes: b"abc".as_slice().into(),
            language: None,
        });
        editor.perform(&Action::TextBox(TextBoxAction::Mode(Mode::Insert)));
        editor.perform(&Action::TextBox(TextBoxAction::Insert('x')));

        editor.perform(&save_to("b.txt"));
        editor.perform(&Action::Scratch(ScratchAction::Save(None)));

        let writes = written(&mut editor);
        assert_eq!(writes[0].0.as_ref(), Path::new("b.txt"));
        assert_eq!(
            writes[1].0.as_ref(),
            Path::new("a.txt"),
            "the document kept its path"
        );
    }

    #[test]
    fn a_copy_leaves_the_document_dirty() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        editor.apply(Input::File {
            path: Path::new("a.txt").into(),
            bytes: b"abc".as_slice().into(),
            language: None,
        });
        editor.perform(&Action::TextBox(TextBoxAction::Mode(Mode::Insert)));
        editor.perform(&Action::TextBox(TextBoxAction::Insert('x')));
        editor.perform(&save_to("b.txt"));

        editor.perform(&Action::TextBox(TextBoxAction::Revert));

        assert!(
            !editor.scratch.buffer().rope().to_string().contains('x'),
            "a revert goes back to the file's text, not to the copy's"
        );
    }

    #[test]
    fn writing_to_a_path_open_as_another_document_is_refused() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        for name in ["a.txt", "b.txt"] {
            editor.apply(Input::File {
                path: Path::new(name).into(),
                bytes: b"abc".as_slice().into(),
                language: None,
            });
        }

        editor.perform(&save_to("a.txt"));

        assert!(
            editor
                .message()
                .is_some_and(|message| message.contains("already open"))
        );
        assert!(written(&mut editor).is_empty(), "nothing written");
    }
}
