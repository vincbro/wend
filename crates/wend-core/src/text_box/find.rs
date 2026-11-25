use super::{ColumnPolicy, Direction, Selection, TextBox, wrap::ScrollOffset};
use crate::{
    config::Target,
    frame::Rect,
    input::{
        action::{Action, FindAction, OverlayAction, OverlayKind},
        key::Key,
    },
    input_box::InputBox,
    layout::{Canvas, Layout},
};

/// What stands before the query.
const PROMPT: &str = "/ ";

/// The prompt a text box searches its own text with, along the bottom of its rect while it is
/// open, floating over the text. A match is scrolled to the middle of the box, clear of it.
///
/// The selection and the scroll the box had when the prompt opened are kept: closing without
/// confirming goes back to them. The prompt is one line of text, which `check_invariants` checks
/// through its input box.
#[derive(Debug, Clone)]
pub(super) struct FindPrompt {
    input: InputBox,
    origin: Selection,
    /// Where the text was scrolled to, to go back to with the selection.
    scroll: ScrollOffset,
    /// What the text was last searched for, to see when it changed.
    searched: String,
    confirmed: bool,
}

impl TextBox {
    /// Opens the prompt of a box that can be searched, and does nothing for one that can't.
    pub(super) fn open_find(&mut self) {
        if !self.searchable || self.find.is_some() {
            return;
        }
        // The config was checked when it was set, and that built the keys of this target too
        let Ok(input) = InputBox::new(
            Layout::FILL,
            &self.config,
            Target::Overlay(OverlayKind::Find),
            PROMPT,
        ) else {
            return;
        };
        self.find = Some(Box::new(FindPrompt {
            input,
            origin: self.selection,
            scroll: self.wrap.scroll_offset(),
            searched: String::new(),
            confirmed: false,
        }));
    }

    /// Feeds `key` to the prompt, and searches again when the query changed. The actions the
    /// prompt doesn't own are added to `requests`, which this clears first.
    pub(super) fn find_key_press(&mut self, key: Key, requests: &mut Vec<Action>) {
        let Some(find) = self.find.as_deref_mut() else {
            return;
        };
        find.input.key_press(key, requests);
        let mut close = false;
        requests.retain(|action| match action {
            Action::Overlay(OverlayAction::Find(FindAction::Confirm)) => {
                find.confirmed = true;
                close = true;
                false
            }
            Action::Overlay(OverlayAction::Find(FindAction::Close)) => {
                close = true;
                false
            }
            _ => true,
        });
        let text = find.input.to_text();
        let changed = text != find.searched;
        if changed {
            find.searched.clone_from(&text);
        }
        let origin = find.origin;
        if close {
            self.close_find();
        } else if changed {
            self.find_from(&text, origin);
        }
    }

    /// Closes the prompt. A confirmed one keeps the match selected and the query for `n` and
    /// `p`, and any other puts the selection back.
    fn close_find(&mut self) {
        let Some(find) = self.find.take() else {
            return;
        };
        if find.confirmed {
            self.set_query(&find.searched);
        } else {
            // The view is put back too, which the search scrolled to show its matches
            self.rebuild_at(find.scroll);
            self.set_selection(find.origin, ColumnPolicy::Sync);
        }
        self.check_invariants();
    }

    /// The rect the prompt takes along the bottom, over the text, none while it is closed.
    pub(super) fn prompt_rect(&self) -> Option<Rect> {
        self.find.as_ref()?;
        Some(self.rect.split_bottom(InputBox::HEIGHT).0)
    }

    #[must_use]
    pub fn is_finding(&self) -> bool {
        self.find.is_some()
    }

    /// Whether the text on `row` is under the prompt, where it can't be read.
    fn under_prompt(&self, row: usize) -> bool {
        self.prompt_rect()
            .is_some_and(|prompt| row >= usize::from(prompt.row - self.rect.row))
    }

    /// Selects `selection`, a match, and scrolls to put it in the middle of the box when it was
    /// off screen or under the prompt. One that is already in view stays where it is, so the
    /// text doesn't jump for a match that is already readable.
    pub(super) fn show_match(&mut self, selection: Selection) {
        self.refresh_wrap();
        let covered = self
            .wrap
            .find_byte(selection.head())
            .is_none_or(|pos| self.under_prompt(pos.row));
        self.set_selection(selection, ColumnPolicy::Sync);
        if covered {
            self.center_head();
        }
        self.check_invariants();
    }

    /// Scrolls so the head is on the middle row, as far as the text allows: a head near the start
    /// or the end of the text stays where it is.
    fn center_head(&mut self) {
        let Some(pos) = self.head_position() else {
            return;
        };
        let middle = self.rect.area.height() / 2;
        if pos.row > middle {
            self.scroll(Direction::Forward, pos.row - middle);
        } else {
            self.scroll(Direction::Backward, middle - pos.row);
        }
    }

    /// Draws the prompt into its rect.
    pub(super) fn render_find(&mut self, canvas: &mut Canvas<'_>) {
        let Some(rect) = self.prompt_rect() else {
            return;
        };
        if let Some(find) = self.find.as_deref_mut() {
            canvas.child(&mut find.input, rect);
        }
    }
}

impl FindPrompt {
    /// Checks the rules of the prompt's text box. Debug builds only.
    pub(super) fn check_invariants(&self) {
        self.input.check_invariants();
    }
}
