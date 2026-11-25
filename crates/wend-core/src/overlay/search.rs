use std::path::Path;

use crop::Rope;

use crate::{
    buffer::Buffer,
    config::{Config, Target},
    frame::{Area, Rect},
    input::{
        Mode,
        action::{Action, EditorAction, OverlayAction, OverlayKind, SearchAction, SearchKind},
        key::Key,
    },
    layout::{Canvas, Element, Fit, Layout, Pane, Size},
    output::Output,
    text_box::{Result, TextBox, input_box::InputBox},
};

/// What stands before the query.
const PROMPT: &str = "> ";

/// An input box to type a query in, with a read-only box of what the frontend found for it under
/// it, over the middle of the editor.
///
/// The core finds nothing itself. Each time the query changes it asks the frontend with
/// `Output::Search` and shows what `results` is given, as long as it answers the newest query.
/// Picking closes the search and opens the file on the line the results' cursor is on.
///
/// Focus has no state of its own: the query has it while it is in insert mode and the results
/// have it otherwise. So `i` in the results and `Esc` in the query are what move between them.
///
/// Both boxes answer to `[keys.search.<mode>]`.
///
/// After every public call the results are read-only and not in insert mode, since the focus goes
/// to the query when they are, and a pick waits only while the answer to the newest search is
/// missing. `check_invariants` checks them.
#[derive(Debug)]
pub(crate) struct SearchOverlay {
    /// What the search looks for.
    kind: SearchKind,
    query: InputBox,
    results: Pane<TextBox>,
    /// The query the newest search was asked for, to see when the text changed.
    asked: Rope,
    /// The number of the newest search, which the answer to it carries.
    latest: u64,
    /// Whether `results` holds the answer to the newest search.
    answered: bool,
    /// Whether the user picked before the answer to the newest search came, which is then picked
    /// from as soon as it does.
    picking: bool,
    /// The search to ask the frontend for, until the editor takes it.
    ask: Option<Output>,
}

impl SearchOverlay {
    /// Opens with an empty query, which is asked for too, so the frontend can list what it
    /// likes before anything is typed.
    pub(crate) fn new(config: &Config, kind: SearchKind) -> Result<Self> {
        let target = Target::Overlay(OverlayKind::Search);
        let query = InputBox::new(Layout::FILL, config, target, PROMPT)?;
        let mut results = TextBox::new(Rect::new(0, 0, Area::new(0, 0)), config, target)?;
        results.set_read_only(true);
        results.set_searchable(true);
        let mut overlay = Self {
            kind,
            query,
            results: Pane::new(Layout::FILL, results),
            asked: Rope::new(),
            latest: 0,
            answered: false,
            picking: false,
            ask: None,
        };
        overlay.ask();
        Ok(overlay)
    }

    /// Asks the frontend to search for what the query says now.
    fn ask(&mut self) {
        self.latest += 1;
        self.answered = false;
        self.picking = false;
        self.asked = self.query.text_box().buffer().rope().clone();
        self.ask = Some(Output::Search {
            id: self.latest,
            kind: self.kind,
            query: self.asked.to_string().into(),
        });
    }

    /// The search to ask the frontend for, if there is one.
    pub(crate) fn take_ask(&mut self) -> Option<Output> {
        self.ask.take()
    }

    /// Feeds `key` to the box that has the focus and runs what it asked for: picking is dealt
    /// with here, and the editor's requests are left in `requests`, which this clears first.
    pub(crate) fn key_press(&mut self, key: Key, requests: &mut Vec<Action>) {
        if self.query.has_focus() {
            self.query.key_press(key, requests);
        } else {
            self.results.inner_mut().key_press(key, requests);
        }
        // `i` in the results moves the focus to the query, and the results are never typed in
        if self.results.inner().mode() == Mode::Insert {
            self.results.inner_mut().set_mode(Mode::Normal);
            self.query.set_mode(Mode::Insert);
        }
        let pick = Action::Overlay(OverlayAction::Search(SearchAction::Pick));
        if requests.contains(&pick) {
            requests.retain(|action| *action != pick);
            self.pick(requests);
        }
        if self.query.text_box().buffer().rope() != &self.asked {
            self.ask();
        }
    }

    /// Takes in the lines the frontend found for search `id`, which are dropped when a newer
    /// search has been asked since. Picks from them when the user already did.
    pub(crate) fn found(&mut self, id: u64, text: &str, requests: &mut Vec<Action>) {
        requests.clear();
        if id != self.latest {
            return;
        }
        // What was shown before is only ever the owner's to replace
        drop(self.results.inner_mut().open_buffer(Buffer::from(text)));
        self.answered = true;
        if self.picking {
            self.picking = false;
            self.pick(requests);
        }
    }

    /// The search `id` failed, so nothing it was asked for is waited for.
    pub(crate) fn failed(&mut self, id: u64) {
        if id == self.latest {
            self.picking = false;
        }
    }

    /// Opens the file on the line the results' cursor is on and closes the search, or waits for
    /// the results when they are not the answer to the newest query.
    fn pick(&mut self, requests: &mut Vec<Action>) {
        if !self.answered {
            self.picking = true;
            return;
        }
        let line = self.results.inner().head_line_text();
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        requests.push(Action::Editor(EditorAction::CloseOverlay));
        requests.push(Action::Editor(match self.kind {
            SearchKind::Files => EditorAction::Open(Path::new(line).into()),
            SearchKind::Text => text_result(line),
        }));
    }

    /// Checks the rules of both boxes. Debug builds only.
    pub(crate) fn check_invariants(&self) {
        self.query.check_invariants();
        let results = self.results.inner();
        results.check_invariants();
        assert!(
            results.is_read_only(),
            "the results are the frontend's, and are never typed in"
        );
        assert!(
            results.mode() != Mode::Insert,
            "key_press hands the focus to the query when the results go to insert mode"
        );
        assert!(
            !self.picking || !self.answered,
            "a pick waits for the answer, and is made as soon as it is there"
        );
    }
}

/// The action for a line of a text search, `path:line:text`: the file opened at the line. A line
/// that isn't in that form is taken as a path.
fn text_result(line: &str) -> EditorAction {
    let at = line.split_once(':').and_then(|(path, rest)| {
        let (number, _) = rest.split_once(':')?;
        Some((path, number.parse().ok()?))
    });
    match at {
        Some((path, line)) => EditorAction::OpenAt {
            path: Path::new(path).into(),
            line,
        },
        None => EditorAction::Open(Path::new(line).into()),
    }
}

impl Element for SearchOverlay {
    fn layout(&self) -> Layout {
        Layout::Float(Fit::centered(Size::Percent(60), Size::Percent(60)))
    }

    /// The input box is the top, and the results are what is left under it. The box without the
    /// focus is drawn first, since the cursor is where the last box put it.
    fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
        let (query, results) = rect.split_top(InputBox::HEIGHT);
        if self.query.has_focus() {
            canvas.child(&mut self.results, results);
            canvas.child(&mut self.query, query);
        } else {
            canvas.child(&mut self.query, query);
            canvas.child(&mut self.results, results);
        }
    }
}
