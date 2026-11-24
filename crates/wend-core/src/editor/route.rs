use std::mem;

use super::{Editor, Overlay, Result, TestOverlay, overlay::KeyOutcome};
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

    /// Does `action` without drawing, so the next frame shows it. The editor's own actions
    /// run whatever is open, the others go to the overlay when one is, else to the view.
    pub fn perform(&mut self, action: Action) -> Result<()> {
        let result = match action {
            Action::Editor(action) => self.perform_editor(action),
            Action::View(action) => {
                match &mut self.overlay {
                    Some(overlay) => overlay.perform(action),
                    None => self.view.perform(action),
                }
                Ok(())
            }
        };
        self.check_invariants();
        result
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Config,
        frame::Area,
        input::{Mode, action::ViewAction},
    };

    #[test]
    fn an_open_overlay_takes_the_view_actions() {
        let mut editor = Editor::new(Area::new(80, 24), &Config::default()).unwrap();
        editor
            .perform(Action::Editor(EditorAction::OpenTestOverlay))
            .unwrap();

        editor
            .perform(Action::View(ViewAction::Mode(Mode::Insert)))
            .unwrap();

        assert_eq!(
            editor.mode(),
            Mode::Normal,
            "the view under it didn't see it"
        );
    }
}
