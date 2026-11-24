use std::mem;

use super::{Editor, Overlay, Result, TestOverlay};
use crate::{
    input::{
        action::{Action, EditorAction},
        key::Key,
    },
    output::Output,
};

/// Where keys and actions go: to the overlay while one is open, to the view otherwise. What the
/// target asks of the editor itself (save, quit, opening an overlay) runs here, on the state.
impl Editor {
    pub(super) fn key_press(&mut self, key: Key) -> Result<()> {
        // An open overlay is first in line for the keys, and takes all of them
        match &mut self.overlay {
            Some(overlay) => overlay.key_press(key, &mut self.requests),
            None => self.view.key_press(key, &mut self.requests),
        }
        // Taken out so the requests can run on the editor, then given back for their memory
        let mut requests = mem::take(&mut self.requests);
        let result = requests
            .drain(..)
            .try_for_each(|action| self.perform_editor(&action));
        self.requests = requests;
        result
    }

    /// Does `action` without drawing, so the next frame shows it. The editor's own actions
    /// run whatever is open, the others go to the overlay when one is, else to the view.
    pub fn perform(&mut self, action: &Action) -> Result<()> {
        let result = match action {
            Action::Editor(action) => self.perform_editor(action),
            Action::View(action) => {
                match &mut self.overlay {
                    Some(overlay) => overlay.perform(*action),
                    None => self.view.perform(*action),
                }
                Ok(())
            }
            Action::Overlay(action) => {
                if let Some(overlay) = &mut self.overlay {
                    overlay.act(*action);
                }
                Ok(())
            }
        };
        self.check_invariants();
        result
    }

    fn perform_editor(&mut self, action: &EditorAction) -> Result<()> {
        match action {
            EditorAction::Save => {
                let (path, rope) = self.view.save()?;
                self.outputs.push(Output::Write { path, rope });
            }
            EditorAction::Open(path) => self.outputs.push(Output::Read { path: path.clone() }),
            EditorAction::CloseOverlay => self.overlay = None,
            EditorAction::OpenTestOverlay => {
                self.overlay = Some(Overlay::Test(TestOverlay::new(&self.config)?));
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
            action::{Scope, ViewAction},
            key::KeyCode,
        },
    };

    #[test]
    fn an_open_overlay_takes_the_view_actions() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        editor
            .perform(&Action::Editor(EditorAction::OpenTestOverlay))
            .unwrap();

        editor
            .perform(&Action::View(ViewAction::Mode(Mode::Insert)))
            .unwrap();

        assert_eq!(
            editor.mode(),
            Mode::Normal,
            "the view under it didn't see it"
        );
    }

    #[test]
    fn open_asks_the_frontend_to_read_the_file() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        let action = Action::parse(Scope::View, "open src/main.rs").unwrap();

        editor.perform(&action).unwrap();

        assert!(
            editor.drain_outputs().any(
                |output| matches!(output, Output::Read { path } if &*path == Path::new("src/main.rs"))
            ),
            "the path is what was typed"
        );
    }

    fn editor_with_overlay() -> Editor {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        editor
            .perform(&Action::Editor(EditorAction::OpenTestOverlay))
            .unwrap();
        editor
    }

    fn press(editor: &mut Editor, code: KeyCode) {
        editor.apply(Input::Key(Key::new(code))).unwrap();
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
        assert_eq!(editor.mode(), Mode::Normal, "i didn't reach the view");
    }

    #[test]
    fn the_overlay_follows_the_keys_of_the_config_it_opened_with() {
        let config = Config::parse("[keys.test]\n\"<esc>\" = []\nx = \"close_overlay\"\n").unwrap();
        let mut editor = Editor::new(Area::new(80, 24), &config).unwrap();
        editor
            .perform(&Action::Editor(EditorAction::OpenTestOverlay))
            .unwrap();

        press(&mut editor, KeyCode::Esc);
        assert!(editor.overlay.is_some(), "esc was unbound");
        press(&mut editor, KeyCode::Char('x'));
        assert!(editor.overlay.is_none(), "x closes it now");
    }
}
