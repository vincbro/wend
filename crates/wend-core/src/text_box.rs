use std::{mem, num::NonZeroUsize, sync::Arc};

use crate::{
    buffer::Buffer,
    config::{Config, Target},
    frame::{CellPosition, Rect},
    input::{
        Mode,
        action::{Action, TextBoxAction},
        key::Key,
    },
    number,
    syntax::{Grammar, Highlight, LanguageTag, Loader},
};

mod edit;
mod error;
mod find;
pub use error::{Error, Result};
use find::FindPrompt;
mod grapheme;
mod highlight;
pub(crate) mod input_box;
use highlight::Highlighter;
mod wrap;
use wrap::{Glyph, ScrollOffset, Slot, SlotPosition, Wrap};
mod motion;
use motion::WordClass;
pub use motion::{Direction, Motion};
mod render;
mod search;
use search::Searching;
mod selection;
pub use selection::Selection;
mod limits;
use limits::Limits;
mod viewport;
use viewport::Viewport;
mod keymap;
use keymap::Keymap;
#[cfg(test)]
pub(crate) mod fuzz;

/// Decides what a selection change does to the column up and down aim for.
#[derive(Debug, Clone, Copy)]
enum ColumnPolicy {
    /// Keeps aiming for the same column, for moving up and down through short lines.
    Keep,
    /// Aims for the column the head is on now.
    Sync,
}

/// Shows a buffer in a rectangle of the frame, with a selection, and fills all of it with text.
///
/// After every public call both ends of the selection sit on grapheme starts, the head
/// is on screen and `desired_col` is inside the width. A read-only box holds the text its owner
/// last gave it, whatever keys it was given. While the prompt that searches the text is open it
/// holds the keys, and the selection it goes back to is kept. `check_invariants` checks them.
#[derive(Debug, Clone)]
pub struct TextBox {
    buffer: Buffer,
    rect: Rect,
    /// What the keys of the prompt and the settings of its text are made from.
    config: Config,
    /// The prompt that searches the text, while it is open.
    find: Option<Box<FindPrompt>>,
    viewport: Viewport,
    selection: Selection,
    /// Scratch space for `render`, kept so that its memory is reused.
    open_highlights: Vec<(usize, Highlight)>,
    /// Scratch space for `word_backward`, kept so that its memory is reused.
    word_scratch: Vec<(usize, WordClass)>,

    desired_col: usize,

    /// How many lines the text may have, and whether the user may change it. Only the owner
    /// changes a read-only box's text, with `open_buffer`, `update` and `limit_lines`.
    limits: Limits,
    searching: Searching,

    mode: Mode,
    /// The keys of who the box is for and the combo being typed, so each text box answers to
    /// its own keys.
    keymap: Keymap,
}

impl TextBox {
    /// Fails when the keys of `config` for `target` can't make key trees.
    pub fn new(rect: Rect, config: &Config, target: Target) -> Result<Self> {
        let buffer = Buffer::new();
        let mut text_box = Self {
            buffer,
            rect,
            config: config.clone(),
            find: None,
            viewport: Viewport::new(rect.area, config.text.tab_width, config.scroll.margin),
            selection: Selection::point(0),
            open_highlights: vec![],
            word_scratch: vec![],

            desired_col: 0,

            limits: Limits::default(),
            searching: Searching::default(),

            mode: Mode::Normal,
            keymap: Keymap::new(&config.keys, target)?,
        };
        // `resize` does nothing for the rect the text box already has
        text_box.refresh_wrap();
        Ok(text_box)
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// Feeds `key` to the text box's keys and runs the actions of the text box's own group. The
    /// others are added to `requests` for their owners to run, which this clears first.
    pub fn key_press(&mut self, key: Key, requests: &mut Vec<Action>) {
        requests.clear();
        if self.find.is_some() {
            self.find_key_press(key, requests);
            return;
        }
        // The keymap stays borrowed while it is read, so the actions are copied out first, so
        // the text box can run them
        let pressed = self.keymap.press_copied(self.mode, key);
        for action in &pressed {
            match action {
                Action::TextBox(TextBoxAction::InsertTyped) => self.insert_typed(key),
                Action::TextBox(action) => self.perform(*action),
                Action::Editor(_) | Action::Scratch(_) | Action::Overlay(_) => {
                    requests.push(action.clone());
                }
            }
        }
        self.keymap.give_back(pressed);
    }

    /// Inserts the character `key` types, nothing for a key that types none.
    fn insert_typed(&mut self, key: Key) {
        if let Some(character) = key.character() {
            self.insert(character);
        }
    }

    /// Does what `action` says to the text, the selection or the mode.
    pub fn perform(&mut self, action: TextBoxAction) {
        match action {
            TextBoxAction::Motion(motion) => {
                self.move_selection(motion, matches!(self.mode, Mode::Select));
            }
            TextBoxAction::Mode(mode) => self.mode = mode,
            TextBoxAction::CollapseSelection => self.collapse_selection(),
            TextBoxAction::Insert(c) => self.insert(c),
            // Needs the key that ran it, which only `key_press` has
            TextBoxAction::InsertTyped => {}
            TextBoxAction::Find => self.open_find(),
            TextBoxAction::Delete => self.delete(),
            TextBoxAction::DeleteSelection => self.delete_selection(),
            TextBoxAction::SelectRow => self.select_row(),
            TextBoxAction::OpenLine => self.new_line(),
            TextBoxAction::Revert => self.revert(),
        }
    }

    #[must_use]
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Sets the language of the shown buffer, see `Buffer::set_language`. It leaves the text
    /// alone, so the wrap and the selection stay as they are.
    pub fn set_language(&mut self, language: Option<LanguageTag>) {
        self.buffer.set_language(language);
        self.check_invariants();
    }

    /// Gives the shown buffer `grammar`, see `Buffer::attach`.
    pub(crate) fn attach(&mut self, grammar: &Arc<Grammar>) {
        self.buffer.attach(grammar);
        self.check_invariants();
    }

    /// Brings the shown buffer's syntax up to date with its text, see `Buffer::reparse`.
    pub(crate) fn reparse(&mut self, loader: &mut Loader) -> Result<()> {
        let result = self.buffer.reparse(loader).map_err(Error::from);
        self.check_invariants();
        result
    }

    /// Makes the box refuse every change to its text, or accept them again. Only a change to the
    /// text is refused: the mode, the selection and every motion work as in any box, and what
    /// the owner puts in with `open_buffer` and `update` is not refused either.
    pub fn set_read_only(&mut self, read_only: bool) {
        self.limits.set_read_only(read_only, self.buffer.rope());
    }

    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.limits.is_read_only()
    }

    /// Takes what the owner put in as the text of a read-only box.
    fn owner_changed_text(&mut self) {
        self.limits.owner_changed_text(self.buffer.rope());
    }

    /// Limits the text to `limit` lines, from now on. What is there past it is cut off.
    pub fn limit_lines(&mut self, limit: NonZeroUsize) {
        self.limits.set_line_limit(limit);
        self.clip_to_line_limit();
        self.owner_changed_text();
        self.viewport.mark_dirty();
        self.set_selection(self.selection, ColumnPolicy::Sync);
        self.check_invariants();
    }

    /// Takes in the file's new contents. The selection keeps its bytes as far as the new
    /// text reaches, the same way a revert does.
    pub fn update(&mut self, bytes: &[u8]) -> Result<()> {
        self.buffer.update(bytes)?;
        self.clip_to_line_limit();
        self.owner_changed_text();
        self.viewport.mark_dirty();
        self.set_selection(self.selection, ColumnPolicy::Sync);
        self.check_invariants();
        Ok(())
    }

    /// Collapses the selection onto its head, so only the cursor is left.
    pub fn collapse_selection(&mut self) {
        self.set_selection(Selection::point(self.selection.head()), ColumnPolicy::Keep);
        self.check_invariants();
    }

    /// Selects the screen row the head is on, growing a selection that spans rows.
    pub fn select_row(&mut self) {
        let Some(pos) = self.head_position() else {
            return;
        };
        let (Some(first), Some(last)) = (
            self.viewport.wrap().edge_glyph(pos.row, Direction::Forward),
            self.viewport
                .wrap()
                .edge_glyph(pos.row, Direction::Backward),
        ) else {
            return;
        };
        let start = self.selection.start().min(first.start());
        // Ends are graphemes included, so the last glyph's start, not its end
        let end = self.selection.end().max(last.start());
        self.set_selection(Selection::new(start, end), ColumnPolicy::Sync);
        self.check_invariants();
    }

    /// The text of the line the head is in, without its line break.
    #[must_use]
    pub(crate) fn head_line_text(&self) -> String {
        let rope = self.buffer.rope();
        let line = rope.line_of_byte(self.selection.head());
        if line >= rope.line_len() {
            return String::new();
        }
        rope.line(line)
            .to_string()
            .trim_end_matches(['\r', '\n'])
            .to_owned()
    }

    /// Returns where the head is in the frame, scrolling first so it is on screen, none if
    /// the area is empty. The area comes from the terminal, which can report no size at all.
    pub fn cursor_position(&mut self) -> Option<CellPosition> {
        self.ensure_cursor_visible();
        let pos = self.head_position()?;
        Some(Self::cell_position(self.rect, pos))
    }

    /// Returns the slot the head is in, none if it is off screen.
    fn head_position(&self) -> Option<SlotPosition> {
        self.viewport.wrap().find_byte(self.selection.head())
    }

    /// Returns where a slot of the wrap ends up in the frame.
    fn cell_position(rect: Rect, pos: SlotPosition) -> CellPosition {
        CellPosition::new(
            rect.col + number::exact::<usize, u16>(pos.col),
            rect.row + number::exact::<usize, u16>(pos.row),
        )
    }

    /// The only way the selection changes. Snaps both ends to graphemes, scrolls so the
    /// head is visible and updates the column up and down aim for.
    fn set_selection(&mut self, selection: Selection, policy: ColumnPolicy) {
        self.selection = Selection::new(
            self.grapheme_start(selection.anchor()),
            self.grapheme_start(selection.head()),
        );
        self.ensure_cursor_visible();
        if let ColumnPolicy::Sync = policy {
            self.sync_desired_col();
        }
    }

    /// Makes the column the head is on the one up and down aim for.
    fn sync_desired_col(&mut self) {
        if let Some(pos) = self.head_position() {
            self.desired_col = pos.col;
        }
    }

    /// Follows the scroll, text and line number settings and the keys of `config` from the
    /// next frame on. A config that fails leaves the text box as it was.
    pub fn set_config(&mut self, config: &Config) -> Result<()> {
        self.keymap.set_keys(&config.keys)?;
        self.config.clone_from(config);
        self.viewport
            .set_config(config.scroll.margin, config.text.tab_width);
        self.set_selection(self.selection, ColumnPolicy::Sync);
        self.check_invariants();
        Ok(())
    }

    /// Moves the text box to `rect`, which changes nothing when it is already there.
    pub fn resize(&mut self, rect: Rect) {
        if self.rect == rect {
            return;
        }
        self.rect = rect;
        self.desired_col = self
            .desired_col
            .min(self.rect.area.width().saturating_sub(1));
        self.viewport.mark_dirty();
        self.set_selection(self.selection, ColumnPolicy::Keep);
        self.check_invariants();
    }

    /// Returns how many rows from the top hold text, the rest are below the end of the buffer.
    #[must_use]
    pub(crate) fn rows_with_text(&self) -> usize {
        self.viewport.wrap().last_row().map_or(0, |row| row + 1)
    }

    /// Returns the screen row the head is on and the line it is in, none if it is off screen.
    #[must_use]
    pub(crate) fn head_row_and_line(&self) -> Option<(usize, usize)> {
        let slot = self.head_position()?;
        let metadata = self.viewport.wrap().row_metadata(slot.row)?;
        Some((slot.row, metadata.line))
    }

    /// Returns the rectangle of the frame the text box is in.
    #[must_use]
    pub(crate) fn rect(&self) -> Rect {
        self.rect
    }

    /// Shows `new_buffer` from its start and returns the buffer shown until now.
    pub fn open_buffer(&mut self, mut new_buffer: Buffer) -> Buffer {
        mem::swap(&mut new_buffer, &mut self.buffer);
        self.clip_to_line_limit();
        self.owner_changed_text();
        // The old selection and scroll are byte and line offsets into the old buffer
        self.desired_col = 0;
        self.rebuild_at(ScrollOffset::default());
        self.set_selection(Selection::point(0), ColumnPolicy::Sync);
        new_buffer
    }

    /// Checks the rules every public function has to leave the text box in, and the ones of the
    /// buffer and the wraps it holds. Debug builds only.
    pub(crate) fn check_invariants(&self) {
        if !cfg!(debug_assertions) {
            return;
        }

        self.buffer.check_invariants();
        if let Some(find) = &self.find {
            find.check_invariants();
        }
        self.limits.check_invariants(self.buffer.rope());
        self.keymap.check_invariants();
        self.viewport.check_invariants();

        let rope = self.buffer.rope();
        for byte in [self.selection.anchor(), self.selection.head()] {
            assert!(
                byte <= rope.byte_len() && rope.is_char_boundary(byte),
                "set_selection snaps both ends onto grapheme starts inside the rope, got {byte}"
            );
        }

        if self.rect.area.is_empty() {
            return;
        }
        assert!(
            !self.viewport.is_dirty() && self.head_position().is_some(),
            "ensure_cursor_visible rebuilds the wrap and scrolls the head on screen"
        );
        assert!(
            self.desired_col < self.rect.area.width(),
            "resize and rebuild_at clamp desired_col to the text width, got {}",
            self.desired_col
        );
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        frame::{Area, Content, Frame},
        layout::{Canvas, Element},
        syntax::{self, LanguageTag},
    };

    /// A text box `cols` columns wide and `rows` tall that shows `text`.
    pub(crate) fn text_box(text: &str, cols: u16, rows: u16) -> TextBox {
        let mut text_box = TextBox::new(
            Rect::new(0, 0, Area::new(cols, rows)),
            &Config::default(),
            Target::TextBox,
        )
        .unwrap();
        text_box.open_buffer(Buffer::from(text));
        text_box.refresh_wrap();
        text_box
    }

    /// Renders the text box into a frame that is exactly as big as it is.
    fn rendered(text_box: &mut TextBox) -> Frame {
        let mut frame = Frame::new(text_box.rect.area, None);
        let rect = text_box.rect;
        text_box.render(rect, &mut Canvas::root(&mut frame));
        frame
    }

    /// Every action a user can reach, to run on a box and its read-only twin.
    fn every_action() -> Vec<TextBoxAction> {
        let mut actions = vec![
            TextBoxAction::Insert('x'),
            TextBoxAction::Insert('\n'),
            TextBoxAction::Delete,
            TextBoxAction::DeleteSelection,
            TextBoxAction::OpenLine,
            TextBoxAction::CollapseSelection,
            TextBoxAction::SelectRow,
            TextBoxAction::Revert,
            TextBoxAction::Find,
        ];
        for mode in [Mode::Normal, Mode::Select, Mode::Insert] {
            actions.push(TextBoxAction::Mode(mode));
        }
        for direction in [Direction::Backward, Direction::Forward] {
            actions.extend(
                [
                    Motion::Horizontal,
                    Motion::Vertical,
                    Motion::Word,
                    Motion::Line,
                    Motion::Page,
                    Motion::File,
                    Motion::Match,
                ]
                .map(|motion| TextBoxAction::Motion(motion(direction))),
            );
        }
        actions
    }

    #[test]
    fn a_read_only_box_refuses_exactly_what_changes_the_text_of_its_twin() {
        for action in every_action() {
            for mode in [Mode::Normal, Mode::Select, Mode::Insert] {
                let mut twin = text_box("one\ntwo words\nthree", 20, 5);
                twin.perform(TextBoxAction::Motion(Motion::Vertical(Direction::Forward)));
                twin.set_mode(mode);
                let mut read_only = twin.clone();
                read_only.set_read_only(true);
                let before = read_only.buffer().rope().to_string();

                twin.perform(action);
                read_only.perform(action);
                let after = twin.buffer().rope().to_string();
                let changed = after != before;

                assert_eq!(
                    read_only.buffer().rope().to_string(),
                    before,
                    "{action:?} in {mode:?} changed the text of a read-only box"
                );
                assert!(
                    changed
                        || (read_only.mode(), read_only.selection) == (twin.mode(), twin.selection),
                    "{action:?} in {mode:?} changes no text, so it is not refused"
                );
            }
        }
    }

    #[test]
    fn what_the_owner_puts_in_is_not_refused() {
        let mut text_box = text_box("abc", 20, 5);
        text_box.set_read_only(true);

        text_box.update(b"xyz").unwrap();
        assert_eq!(text_box.buffer().rope().to_string(), "xyz");
        text_box.open_buffer(Buffer::from("new"));

        assert_eq!(text_box.buffer().rope().to_string(), "new");
    }

    #[test]
    fn the_selection_and_the_cursor_are_layers_over_the_text() {
        let mut text_box = text_box("abcdef", 20, 2);
        text_box.set_selection(Selection::new(0, 2), ColumnPolicy::Sync);

        let frame = rendered(&mut text_box);

        let style = |col: u16| frame.get(CellPosition::new(col, 0)).style;
        assert!(style(0).selected && !style(0).cursor, "the a is selected");
        assert!(style(2).selected && style(2).cursor, "the head is both");
        assert!(!style(3).selected && !style(3).cursor, "the d is neither");
    }

    /// The rows of the frame whose cells all carry the current line layer.
    fn rows_on_the_line(frame: &Frame) -> Vec<u16> {
        (0..frame.area().rows)
            .filter(|&row| frame.row(row).iter().all(|cell| cell.style.line))
            .collect()
    }

    #[test]
    fn the_whole_line_of_the_head_carries_the_line_layer_to_the_edge() {
        let mut text_box = text_box("ab\ncccccccc\nd", 4, 8);
        text_box.perform(TextBoxAction::Motion(Motion::Vertical(Direction::Forward)));

        let frame = rendered(&mut text_box);

        assert_eq!(
            rows_on_the_line(&frame),
            [1, 2, 3, 4],
            "every row of the wrapped line, blank cells to the edge included, and no other row"
        );
    }

    #[test]
    fn a_selection_over_the_line_keeps_both_layers() {
        let mut text_box = text_box("abc", 6, 2);
        text_box.set_selection(Selection::new(0, 1), ColumnPolicy::Sync);

        let frame = rendered(&mut text_box);

        let style = frame.get(CellPosition::new(0, 0)).style;
        assert!(
            style.line && style.selected,
            "the layers don't replace each other"
        );
    }

    #[test]
    fn a_wrapped_row_ends_in_a_marker() {
        let mut text_box = text_box("aaaaaaaa", 4, 3);

        let frame = rendered(&mut text_box);

        let markers: Vec<_> = frame
            .iter()
            .filter(|(_, cell)| cell.style.content == Content::Marker)
            .collect();
        assert_eq!(markers.len(), 2, "3, 3 and 2 characters, so two wraps");
        assert_eq!(markers[0].1.grapheme.as_str_sanitize(), "↩");
    }

    #[test]
    fn a_config_sets_the_tab_width() {
        let mut text_box = text_box("\tx", 20, 5);
        text_box.set_selection(Selection::point(1), ColumnPolicy::Sync);
        text_box.ensure_cursor_visible();
        assert_eq!(text_box.head_position().unwrap().col, 4, "a tab is 4 wide");

        let mut config = Config::default();
        config.text.tab_width = 2.try_into().unwrap();
        text_box.set_config(&config).unwrap();
        text_box.ensure_cursor_visible();
        assert_eq!(text_box.head_position().unwrap().col, 2, "a tab is 2 wide");
    }

    #[test]
    fn collapse_selection_keeps_the_head() {
        let mut text_box = text_box("hello world", 20, 5);
        text_box.set_selection(Selection::new(0, 4), ColumnPolicy::Sync);
        text_box.collapse_selection();
        assert_eq!(text_box.selection, Selection::point(4), "the head stays");
    }

    #[test]
    fn set_selection_snaps_and_clamps() {
        // é is bytes 1..3
        let mut text_box = text_box("aé b", 20, 5);
        text_box.set_selection(Selection::point(2), ColumnPolicy::Sync);
        assert_eq!(
            text_box.selection,
            Selection::point(1),
            "snaps into the grapheme start"
        );

        text_box.set_selection(Selection::new(2, 100), ColumnPolicy::Sync);
        assert_eq!(
            text_box.selection,
            Selection::new(1, 5),
            "clamps to the end of the buffer"
        );
        text_box.check_invariants();
    }

    #[test]
    fn grapheme_bounds_from_the_wrap_match_the_rope() {
        // Combining marks, wide glyphs, tabs, CRLF and wrapped rows, most of it off screen
        let text = "e\u{301}a漢\t🦀\r\nb a line long enough to wrap\n\n".repeat(4);
        let text_box = text_box(&text, 8, 6);
        for byte in 0..=text.len() + 1 {
            let start = text_box.grapheme_start(byte);
            assert_eq!(
                start,
                text_box.rope_grapheme_start(byte),
                "start of byte {byte}"
            );
            assert_eq!(
                text_box.grapheme_end(start),
                text_box.rope_grapheme_end(start),
                "end of the grapheme starting at {start}"
            );
        }
    }

    #[test]
    fn update_to_shorter_text_keeps_the_selection_inside() {
        let mut text_box = text_box("hello world", 20, 5);
        text_box.set_selection(Selection::new(6, 10), ColumnPolicy::Sync);
        text_box.update(b"hi").unwrap();

        assert_eq!(text_box.buffer.rope().to_string(), "hi");
        assert_eq!(
            (text_box.selection.anchor(), text_box.selection.head()),
            (2, 2),
            "both ends move back to the end of the new text"
        );
    }

    #[test]
    fn grapheme_start_ignores_an_out_of_date_wrap() {
        let mut text_box = text_box("ab", 20, 5);
        // The mark joins a, the wrap still has b on byte 1
        text_box.buffer.edit(1..1, "\u{301}");
        text_box.viewport.mark_dirty();
        assert_eq!(text_box.grapheme_start(1), 0, "reads the edited text");
    }

    #[test]
    fn render_covers_wide_tails_only() {
        let mut text_box = text_box("a😀b\tc", 20, 2);
        let frame = rendered(&mut text_box);
        let cell = |col: u16| frame.get(CellPosition::new(col, 0));

        assert_eq!(cell(1).grapheme.as_str(), "😀");
        assert!(!cell(1).covered, "the head prints");
        assert!(cell(2).covered, "the emoji already draws its tail");
        assert_eq!(cell(3).grapheme.as_str(), "b");
        assert!(!cell(3).covered);
        // The tab spans cols 4..8, its head prints one blank so the tails must too
        assert!(
            (4..8).all(|col| !cell(col).covered),
            "tab tails print blanks"
        );
        assert_eq!(cell(8).grapheme.as_str(), "c");
    }

    #[test]
    fn render_colors_the_highlighted_text() {
        let (mut loader, grammar) = syntax::tests::rust();
        let mut text_box = text_box("fn main() {}", 20, 2);
        text_box.set_language(Some(LanguageTag::new("rust")));
        text_box.attach(&grammar);
        text_box.reparse(&mut loader).unwrap();

        let frame = rendered(&mut text_box);

        let content = |col: u16| frame.get(CellPosition::new(col, 0)).style.content;
        let keyword = Content::Text(Highlight::Keyword);
        assert_eq!(content(0), keyword, "the f of fn");
        assert_eq!(content(1), keyword, "the n of fn");
        assert_eq!(content(2), Content::Plain, "the space");
        assert_eq!(
            content(3),
            Content::Text(Highlight::Function),
            "the m of main"
        );
    }

    #[test]
    fn the_terminal_cursor_sits_on_the_head() {
        let mut text_box = text_box("abc", 20, 3);

        assert_eq!(text_box.cursor_position(), Some(CellPosition::new(0, 0)));
        text_box.move_selection(Motion::Horizontal(Direction::Forward), false);
        assert_eq!(
            text_box.cursor_position(),
            Some(CellPosition::new(1, 0)),
            "one grapheme in"
        );
    }

    #[test]
    fn the_head_row_and_line_follow_the_wrap() {
        let mut text_box = text_box("aaaaaaaa\nb", 4, 8);
        assert_eq!(text_box.head_row_and_line(), Some((0, 0)));
        assert!(text_box.rows_with_text() > 2, "the first line wraps");

        text_box.perform(TextBoxAction::Motion(Motion::File(Direction::Forward)));

        let (row, line) = text_box.head_row_and_line().unwrap();
        assert_eq!(line, 1, "on the b");
        assert!(row > 1, "below the rows of the first line");
    }

    fn one_line(text: &str) -> TextBox {
        let mut text_box = text_box("", 20, 3);
        text_box.limit_lines(NonZeroUsize::MIN);
        text_box.open_buffer(Buffer::from(text));
        text_box
    }

    #[test]
    fn an_insert_is_cut_at_the_first_line_break_that_goes_over_the_limit() {
        let mut text_box = one_line("");

        text_box.perform(TextBoxAction::Insert('a'));
        text_box.perform(TextBoxAction::Insert('\n'));
        text_box.perform(TextBoxAction::Insert('b'));

        assert_eq!(text_box.buffer.rope().to_string(), "ab", "no second line");
    }

    #[test]
    fn open_line_does_nothing_at_the_limit_and_leaves_the_cursor() {
        let mut text_box = one_line("abc");
        text_box.perform(TextBoxAction::Motion(Motion::File(Direction::Backward)));

        text_box.perform(TextBoxAction::OpenLine);

        assert_eq!(text_box.buffer.rope().to_string(), "abc");
        assert_eq!(text_box.selection, Selection::point(0), "the cursor stayed");
    }

    #[test]
    fn a_buffer_past_the_limit_is_cut_when_it_is_shown() {
        let text_box = one_line("first\r\nsecond\nthird");

        assert_eq!(text_box.buffer.rope().to_string(), "first");
    }

    #[test]
    fn a_revert_is_cut_to_the_limit_too() {
        let mut text_box = one_line("");
        text_box.limit_lines(NonZeroUsize::new(2).unwrap());
        text_box.open_buffer(Buffer::from("a\nb"));
        text_box.limit_lines(NonZeroUsize::MIN);
        text_box.perform(TextBoxAction::Insert('x'));

        text_box.perform(TextBoxAction::Revert);

        assert_eq!(
            text_box.buffer.rope().to_string(),
            "a",
            "not the clean two lines"
        );
    }

    #[test]
    fn a_limit_of_several_lines_allows_breaks_up_to_it() {
        let mut text_box = text_box("", 20, 3);
        text_box.limit_lines(NonZeroUsize::new(2).unwrap());

        for key in ['a', '\n', 'b', '\n', 'c'] {
            text_box.perform(TextBoxAction::Insert(key));
        }

        assert_eq!(
            text_box.buffer.rope().to_string(),
            "a\nbc",
            "the second break is refused"
        );
    }
}
