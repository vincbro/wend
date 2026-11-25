use std::{collections::HashSet, iter, path::Path};

use super::{Error, Result};
use crate::{
    buffer::Buffer,
    scratch::Scratch,
    syntax::{self, LanguageTag, Loader},
    text_box,
};

mod document;
use document::Document;

/// Where a save goes and what it does to the file shown.
#[derive(Debug)]
pub(super) struct SavePlan {
    pub(super) path: Box<Path>,
    /// Whether the save is a copy that leaves the document as dirty as it was.
    pub(super) is_copy: bool,
    /// Whether the save names a document that had no path.
    pub(super) names: bool,
}

/// The files that are open, and the grammars they are written in.
///
/// `documents` holds every open document except the one the scratch shows, whose path is
/// `shown_path`, so each open file lives in exactly one place. The buffer shown is the
/// scratch's, so what changes it takes the scratch.
///
/// After every public call every hidden buffer keeps its own invariants, no path is open twice,
/// and a language that is being waited for isn't loaded yet. `check_invariants` checks them.
#[derive(Debug)]
pub(super) struct Workspace {
    documents: Vec<Document>,
    /// The file the scratch's buffer came from, none for a buffer that never had one.
    shown_path: Option<Box<Path>>,
    loader: Loader,
    /// The languages whose grammar was asked for and hasn't come yet, so a buffer opened
    /// meanwhile doesn't ask again.
    requested_languages: HashSet<LanguageTag>,
    /// The file `open_at` asked for and the line to put the cursor on once it is shown.
    show_at: Option<(Box<Path>, usize)>,
}

impl Workspace {
    pub(super) fn new(loader: Loader) -> Self {
        Self {
            documents: vec![],
            shown_path: None,
            loader,
            requested_languages: HashSet::new(),
            show_at: None,
        }
    }

    /// Shows the file at `path` in `scratch`, reusing its document if it's among the open
    /// documents. The document shown before joins them.
    pub(super) fn show_file(
        &mut self,
        scratch: &mut Scratch,
        path: Box<Path>,
        bytes: &[u8],
    ) -> Result<()> {
        if self.shown_path.as_deref() == Some(&*path) {
            scratch.update(bytes)?;
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
        let hidden = scratch.open_buffer(buffer);
        let hidden_path = self.shown_path.replace(path);
        self.documents.push(Document::new(hidden_path, hidden));
        Ok(())
    }

    /// Remembers to put the cursor on `line` once the file at `path` is shown.
    pub(super) fn show_at(&mut self, path: Box<Path>, line: usize) {
        self.show_at = Some((path, line));
    }

    /// Returns the line `show_at` asked for in the file at `path`, once.
    pub(super) fn take_line(&mut self, path: &Path) -> Option<usize> {
        self.show_at
            .take_if(|(wanted, _)| **wanted == *path)
            .map(|(_, line)| line)
    }

    /// Forgets what was waited for from a read of `path` that failed: the file isn't coming,
    /// so there is no line to go to.
    pub(super) fn read_failed(&mut self, path: &Path) {
        self.take_line(path);
    }

    /// Forgets that the grammar of `tag` was asked for, since it isn't coming and a buffer
    /// opened later may ask again.
    pub(super) fn language_failed(&mut self, tag: &LanguageTag) {
        self.requested_languages.remove(tag);
    }

    /// Works out where a save to `target` goes, the file shown if there is none.
    ///
    /// A save to a path writes there. For an unnamed document that names it, so it is clean and
    /// later saves go there. For a named one it is a copy. A path that is another open
    /// document's is refused, since each path is kept in one place.
    pub(super) fn save_plan(&self, target: Option<&Path>) -> Result<SavePlan> {
        let Some(path) = target.or(self.shown_path.as_deref()) else {
            return Err(Error::NoPath);
        };
        let own = self.shown_path.as_deref() == Some(path);
        let names = self.shown_path.is_none();
        if !own
            && self
                .documents
                .iter()
                .any(|document| document.path() == Some(path))
        {
            return Err(Error::PathOpen(path.into()));
        }
        Ok(SavePlan {
            path: path.into(),
            is_copy: !own && !names,
            names,
        })
    }

    /// Names the document shown, which had no path.
    pub(super) fn name_shown(&mut self, path: Box<Path>) {
        self.shown_path = Some(path);
    }

    /// Reparses the shown buffer if its text changed. A buffer whose parse fails goes back to
    /// plain text, so one bad grammar doesn't stop the editor.
    pub(super) fn reparse(&mut self, scratch: &mut Scratch) -> text_box::Result<()> {
        scratch.reparse(&mut self.loader).inspect_err(|_| {
            scratch.set_language(None);
        })
    }

    /// Sets the language of the shown buffer, and gives it the grammar if that is loaded. If
    /// it isn't, returns the language to ask the frontend for, once, and the buffer stays
    /// plain text until then.
    #[must_use]
    pub(super) fn set_language(
        &mut self,
        scratch: &mut Scratch,
        language: Option<LanguageTag>,
    ) -> Option<LanguageTag> {
        scratch.set_language(language.clone());
        let tag = language?;
        if let Some(grammar) = self.loader.grammar(&tag) {
            scratch.attach(grammar);
            None
        } else {
            self.requested_languages.insert(tag.clone()).then_some(tag)
        }
    }

    /// Loads the grammar the frontend sent and gives it to every buffer written in its
    /// language. A grammar that fails to load leaves those buffers as plain text.
    pub(super) fn load_language(
        &mut self,
        scratch: &mut Scratch,
        tag: LanguageTag,
        wasm: &[u8],
        highlights: &[u8],
    ) -> syntax::Result<()> {
        self.requested_languages.remove(&tag);
        let grammar = self.loader.load(tag, wasm, highlights)?;
        scratch.attach(&grammar);
        for document in &mut self.documents {
            document.attach(&grammar);
        }
        Ok(())
    }
}

impl Workspace {
    /// Checks the rules of what the workspace holds. Debug builds only.
    pub(super) fn check_invariants(&self) {
        if !cfg!(debug_assertions) {
            return;
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

        assert!(
            self.requested_languages
                .iter()
                .all(|tag| self.loader.grammar(tag).is_none()),
            "a language is only asked for while its grammar isn't loaded, and load_language \
             stops waiting for it"
        );
    }
}

#[cfg(test)]
impl Workspace {
    /// Gives back the loader, so the next editor of a fuzz run starts without making one.
    pub(super) fn into_loader(self) -> Loader {
        self.loader
    }

    /// The paths of the documents that are not shown.
    pub(super) fn hidden_paths(&self) -> Vec<Option<&Path>> {
        self.documents.iter().map(Document::path).collect()
    }

    pub(super) fn shown_path(&self) -> Option<&Path> {
        self.shown_path.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Config,
        frame::{Area, Rect},
        syntax,
    };

    #[test]
    fn a_loaded_grammar_reaches_the_buffers_that_are_not_shown() {
        let mut scratch =
            Scratch::new(Rect::new(0, 0, Area::new(80, 24)), &Config::default()).unwrap();
        let mut workspace = Workspace::new(Loader::new().unwrap());
        let text = "fn a() {}";
        let mut hidden = Buffer::from(text);
        hidden.set_language(Some(LanguageTag::new("rust")));
        workspace.documents.push(Document::new(None, hidden));

        workspace
            .load_language(
                &mut scratch,
                LanguageTag::new("rust"),
                syntax::tests::RUST_WASM,
                syntax::tests::RUST_HIGHLIGHTS,
            )
            .unwrap();
        workspace.documents[0]
            .buffer_mut()
            .reparse(&mut workspace.loader)
            .unwrap();

        assert_ne!(
            workspace.documents[0]
                .buffer_mut()
                .highlights(0..text.len())
                .count(),
            0,
            "the buffer got the grammar while it was out of sight"
        );
    }

    #[test]
    fn a_save_to_a_path_open_as_another_document_is_refused() {
        let mut scratch =
            Scratch::new(Rect::new(0, 0, Area::new(80, 24)), &Config::default()).unwrap();
        let mut workspace = Workspace::new(Loader::new().unwrap());
        for name in ["a.txt", "b.txt"] {
            workspace
                .show_file(&mut scratch, Path::new(name).into(), b"abc")
                .unwrap();
        }

        let refused = workspace.save_plan(Some(Path::new("a.txt")));

        assert!(
            matches!(refused, Err(Error::PathOpen(_))),
            "a path kept as another document is refused"
        );
    }

    #[test]
    fn a_save_without_a_target_goes_to_the_shown_path() {
        let mut scratch =
            Scratch::new(Rect::new(0, 0, Area::new(80, 24)), &Config::default()).unwrap();
        let mut workspace = Workspace::new(Loader::new().unwrap());
        assert!(
            matches!(workspace.save_plan(None), Err(Error::NoPath)),
            "nothing is shown from a file, so there is no path to save to"
        );

        workspace
            .show_file(&mut scratch, Path::new("a.txt").into(), b"abc")
            .unwrap();

        let plan = workspace.save_plan(None).unwrap();
        assert_eq!(&*plan.path, Path::new("a.txt"));
        assert!(!plan.is_copy, "a save to the shown path is no copy");
        assert!(!plan.names, "and the document already has a name");
    }
}
