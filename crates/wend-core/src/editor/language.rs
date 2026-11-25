use super::{Editor, Error};
use crate::{output::Output, syntax::LanguageTag};

impl Editor {
    /// Reparses the shown buffer if its text changed. A buffer whose parse fails goes back to
    /// plain text, so one bad grammar doesn't stop the editor.
    pub(super) fn reparse(&mut self) {
        if let Err(error) = self.scratch.reparse(&mut self.loader) {
            self.scratch.set_language(None);
            self.fail(&Error::TextBox(error));
        }
    }

    /// Sets the language of the shown buffer, and gives it the grammar if that is loaded. If
    /// it isn't, asks the frontend for it, once, and the buffer stays plain text until then.
    pub(super) fn set_language(&mut self, language: Option<LanguageTag>) {
        self.scratch.set_language(language.clone());
        let Some(tag) = language else {
            return;
        };
        if let Some(grammar) = self.loader.grammar(&tag) {
            self.scratch.attach(grammar);
        } else if self.requested_languages.insert(tag.clone()) {
            self.outputs.push(Output::Language(tag));
        }
    }

    /// Loads the grammar the frontend sent and gives it to every buffer written in its
    /// language. A grammar that fails to load leaves those buffers as plain text.
    pub(super) fn load_language(&mut self, tag: LanguageTag, wasm: &[u8], highlights: &[u8]) {
        self.requested_languages.remove(&tag);
        let grammar = match self.loader.load(tag, wasm, highlights) {
            Ok(grammar) => grammar,
            Err(error) => {
                self.fail(&Error::Syntax(error));
                return;
            }
        };
        self.scratch.attach(&grammar);
        for document in &mut self.documents {
            document.attach(&grammar);
        }
    }
}
