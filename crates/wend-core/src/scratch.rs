use std::sync::Arc;

use crop::Rope;

use crate::{
    buffer::Buffer,
    config::{Config, LineNumbers, Target},
    frame::{CellPosition, Rect},
    input::{
        Mode,
        action::{Action, ScratchAction, TextBoxAction},
        key::Key,
    },
    layout::{Canvas, Element, Fit, Layout, Size},
    syntax::{Grammar, LanguageTag, Loader},
    text_box::{self, TextBox},
};

mod gutter;
use gutter::Gutter;
#[cfg(test)]
mod fuzz;

/// The columns of text a text box is wide at most, until each language has its own.
const MAX_TEXT_WIDTH: u16 = 100;

/// The surface a document is edited on: a `TextBox` for the text, with the line numbers at its
/// left, both centered in the rect it fills.
///
/// The width of the gutter follows the line count, so after every call that can change the
/// text it fits the text box to what the gutter leaves. `check_invariants` checks it did.
#[derive(Debug, Clone)]
pub struct Scratch {
    text_box: TextBox,
    rect: Rect,
    gutter: Gutter,
    line_numbers: LineNumbers,
}

impl Scratch {
    /// Fails when the keys of `config` can't make key trees.
    pub fn new(rect: Rect, config: &Config) -> text_box::Result<Self> {
        let mut text_box = TextBox::new(rect, config, Target::Scratch)?;
        text_box.set_searchable(true);
        let mut scratch = Self {
            gutter: Self::gutter_for(&text_box, rect, config.line_numbers),
            text_box,
            rect,
            line_numbers: config.line_numbers,
        };
        scratch.fit();
        scratch.check_invariants();
        Ok(scratch)
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.text_box.mode()
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.text_box.set_mode(mode);
    }

    #[must_use]
    pub fn buffer(&self) -> &Buffer {
        self.text_box.buffer()
    }

    /// Feeds `key` to the text box's keys, see `TextBox::key_press`.
    ///
    /// The gutter is fitted once the whole chain of actions has run, so an action that is
    /// one of several can still run at the width from before the one that grew the gutter.
    pub fn key_press(&mut self, key: Key, requests: &mut Vec<Action>) {
        self.text_box.key_press(key, requests);
        self.fit();
        self.check_invariants();
    }

    /// Does what `action` says to the text, the selection or the mode.
    pub fn perform(&mut self, action: TextBoxAction) {
        self.text_box.perform(action);
        self.fit();
        self.check_invariants();
    }

    /// Does what `action` says about the document. Returns the text to write when there is
    /// some. A save to the document's own file makes it the text a revert goes back to. A save
    /// to another file (`copy`) leaves the document as it was.
    pub fn act(&mut self, action: &ScratchAction, copy: bool) -> Option<Rope> {
        match action {
            ScratchAction::Save(_) if copy => Some(self.text_box.text()),
            ScratchAction::Save(_) => Some(self.text_box.save()),
        }
    }

    /// Puts the cursor on `line`, counted from 1, see `TextBox::go_to_line`.
    pub fn go_to_line(&mut self, line: usize) {
        self.text_box.go_to_line(line);
        self.check_invariants();
    }

    /// Sets the language of the shown buffer, see `Buffer::set_language`.
    pub fn set_language(&mut self, language: Option<LanguageTag>) {
        self.text_box.set_language(language);
    }

    /// Gives the shown buffer `grammar`, see `Buffer::attach`.
    pub(crate) fn attach(&mut self, grammar: &Arc<Grammar>) {
        self.text_box.attach(grammar);
    }

    /// Brings the shown buffer's syntax up to date with its text, see `Buffer::reparse`.
    pub(crate) fn reparse(&mut self, loader: &mut Loader) -> text_box::Result<()> {
        self.text_box.reparse(loader)
    }

    /// Takes in the file's new contents, see `TextBox::update`.
    pub fn update(&mut self, bytes: &[u8]) -> text_box::Result<()> {
        let result = self.text_box.update(bytes);
        self.fit();
        self.check_invariants();
        result
    }

    /// Shows `new_buffer` from its start and returns the buffer shown until now.
    pub fn open_buffer(&mut self, new_buffer: Buffer) -> Buffer {
        let old_buffer = self.text_box.open_buffer(new_buffer);
        self.fit();
        self.check_invariants();
        old_buffer
    }

    /// Follows the scroll, text and line number settings and the keys of `config` from the
    /// next frame on. A config that fails leaves the scratch as it was.
    pub fn set_config(&mut self, config: &Config) -> text_box::Result<()> {
        self.text_box.set_config(config)?;
        self.line_numbers = config.line_numbers;
        self.fit();
        self.check_invariants();
        Ok(())
    }

    /// Moves the scratch to `rect`, which changes nothing when it is already there.
    pub fn resize(&mut self, rect: Rect) {
        self.rect = rect;
        self.fit();
        self.check_invariants();
    }

    /// Returns the rectangle of the frame the scratch is in.
    #[must_use]
    pub(crate) fn rect(&self) -> Rect {
        self.rect
    }

    /// Fits the text box to the part of the scratch the gutter leaves for it.
    fn fit(&mut self) {
        self.gutter = Self::gutter_for(&self.text_box, self.rect, self.line_numbers);
        self.text_box.resize(self.text_rect());
    }

    /// Returns the gutter `text_box` needs in a scratch at `rect`.
    fn gutter_for(text_box: &TextBox, rect: Rect, line_numbers: LineNumbers) -> Gutter {
        // The cursor can sit on the empty line after a final line break, which the rope does
        // not count, so the last line number is one more
        let line_count = text_box.buffer().rope().line_len() + 1;
        Gutter::new(
            line_count.max(rect.area.height()),
            rect.area.cols,
            line_numbers,
        )
    }

    /// Returns the part of the scratch the gutter and the text take: the gutter and a full
    /// width of text, centered in the scratch, or all of it when that is narrower.
    fn block(&self) -> Rect {
        let width = self.gutter.width() + MAX_TEXT_WIDTH;
        Layout::Fit(Fit::centered(Size::Fixed(width), Size::Fill))
            .place(self.rect, self.rect)
            .0
    }

    /// Returns the part of the scratch the text is laid out in, to the right of the gutter.
    fn text_rect(&self) -> Rect {
        self.gutter.split(self.block()).1
    }

    /// Returns where the head is in the frame, none if the area is empty, see
    /// `TextBox::cursor_position`.
    pub fn cursor_position(&mut self) -> Option<CellPosition> {
        self.text_box.cursor_position()
    }

    /// Checks the rules every public function has to leave the scratch in, and the ones of the
    /// text box it holds. Debug builds only.
    pub(crate) fn check_invariants(&self) {
        if !cfg!(debug_assertions) {
            return;
        }

        self.text_box.check_invariants();
        assert_eq!(
            self.text_box.rect(),
            self.text_rect(),
            "fit gives the text box what the gutter leaves"
        );
        assert_eq!(
            self.gutter.width(),
            Self::gutter_for(&self.text_box, self.rect, self.line_numbers).width(),
            "fit makes the gutter as wide as the line count needs"
        );
    }
}

impl Element for Scratch {
    /// Takes all the room it is given, and centers the text in it itself.
    fn layout(&self) -> Layout {
        Layout::FILL
    }

    /// Writes the gutter and the text into `canvas`, and puts the cursor where the head is.
    fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
        self.resize(rect);
        let text_rect = self.text_rect();
        canvas.child(&mut self.text_box, text_rect);
        let head = self
            .text_box
            .head_row_and_line()
            .map(|(row, line)| gutter::Head { row, line });
        self.gutter
            .render(canvas, self.block(), self.text_box.rows_with_text(), head);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        buffer::Buffer,
        frame::{Area, CellPosition, Frame},
        input::key::{Key, KeyCode},
        number,
        text_box::{Direction, Motion},
    };

    /// A scratch whose text is `cols` columns wide, with the gutter on top of that.
    fn scratch(text: &str, cols: u16, rows: u16) -> Scratch {
        // Wide enough that the gutter takes what it wants, whatever `cols` is
        let mut scratch = Scratch::new(
            Rect::new(0, 0, Area::new(cols + 100, rows)),
            &Config::default(),
        )
        .unwrap();
        scratch.open_buffer(Buffer::from(text));
        let gutter = scratch.gutter.width();
        scratch.resize(Rect::new(0, 0, Area::new(cols + gutter, rows)));
        scratch
    }

    /// Renders the scratch into a frame that is exactly as big as it is.
    fn rendered(scratch: &mut Scratch) -> Frame {
        let mut frame = Frame::new(scratch.rect.area, None);
        let rect = scratch.rect;
        scratch.render(rect, &mut Canvas::root(&mut frame));
        frame
    }

    /// What the gutter shows on `row` of `frame`.
    fn gutter_text(scratch: &Scratch, frame: &Frame, row: u16) -> String {
        frame.row(row)[..usize::from(scratch.gutter.width())]
            .iter()
            .map(|cell| cell.grapheme.as_str_sanitize())
            .collect()
    }

    #[test]
    fn a_config_sets_the_gutter() {
        let mut scratch = scratch("x", 20, 5);
        let gutter = scratch.gutter.width();

        let mut config = Config::default();
        config.line_numbers.min_digits = 5;
        scratch.set_config(&config).unwrap();

        assert_eq!(
            scratch.gutter.width(),
            gutter + 2,
            "the numbers make room for two more digits"
        );
    }

    #[test]
    fn rows_are_numbered_by_their_distance_from_the_cursor() {
        let mut scratch = scratch(&"x\n".repeat(20), 20, 12);
        for _ in 0..5 {
            scratch.perform(TextBoxAction::Motion(Motion::Vertical(Direction::Forward)));
        }

        let frame = rendered(&mut scratch);

        let shown: Vec<_> = (0..12)
            .map(|row| gutter_text(&scratch, &frame, row))
            .collect();
        assert_eq!(
            shown,
            [
                "  5 ", "  4 ", "  3 ", "  2 ", "  1 ", "  6 ", "  1 ", "  2 ", "  3 ", "  4 ",
                "  5 ", "  6 "
            ],
            "the cursor is on row 5 of line 6, and the rest count rows away from it"
        );
    }

    #[test]
    fn wrapped_rows_count_like_any_other_row() {
        let mut scratch = scratch("aaaaaaaa\nb", 4, 8);

        let frame = rendered(&mut scratch);

        let rows_with_text = scratch.text_box.rows_with_text();
        assert!(rows_with_text > 2, "the first line wraps");
        for row in 1..rows_with_text {
            assert_eq!(
                gutter_text(&scratch, &frame, number::exact(row)),
                format!("{row:>3} "),
                "row {row} is {row} motions below the cursor, wrapped or not"
            );
        }
    }

    #[test]
    fn the_rows_below_the_end_of_the_file_have_no_number() {
        let mut scratch = scratch("a\nb", 20, 6);

        let frame = rendered(&mut scratch);

        assert_eq!(
            gutter_text(&scratch, &frame, 0),
            "  1 ",
            "the cursor's line"
        );
        assert_eq!(gutter_text(&scratch, &frame, 1), "  1 ", "one row down");
        for row in 2..6 {
            assert_eq!(
                gutter_text(&scratch, &frame, row),
                "    ",
                "you can't move onto row {row}, so it has no number"
            );
        }
    }

    #[test]
    fn the_terminal_cursor_sits_in_the_text_and_not_the_gutter() {
        let mut scratch = scratch("abc", 20, 3);
        let gutter = scratch.gutter.width();

        assert_eq!(
            scratch.cursor_position(),
            Some(CellPosition::new(gutter, 0)),
            "right of the gutter"
        );
        scratch.perform(TextBoxAction::Motion(Motion::Horizontal(
            Direction::Forward,
        )));
        assert_eq!(
            scratch.cursor_position(),
            Some(CellPosition::new(gutter + 1, 0)),
            "one grapheme in"
        );
    }

    #[test]
    fn the_gutter_widens_past_999_lines_and_the_text_gives_up_a_column() {
        let mut scratch = scratch(&"a\n".repeat(998), 20, 5);
        assert_eq!(scratch.gutter.width(), 4, "999 lines fit three digits");
        assert_eq!(scratch.text_rect().area.cols, 20);

        scratch.perform(TextBoxAction::OpenLine);

        assert_eq!(scratch.gutter.width(), 5, "1000 lines need a fourth digit");
        assert_eq!(
            scratch.text_rect().area.cols,
            19,
            "the text gave up a column"
        );
    }

    #[test]
    fn a_key_chain_that_grows_the_gutter_leaves_the_text_box_fitted() {
        let mut scratch = scratch(&"a\n".repeat(998), 20, 5);
        let mut requests = vec![];

        // Opens a line and goes into insert mode, which are two actions of one key press
        scratch.key_press(Key::new(KeyCode::Char('o')), &mut requests);

        assert_eq!(scratch.mode(), Mode::Insert, "the whole chain ran");
        assert_eq!(scratch.gutter.width(), 5, "1000 lines need a fourth digit");
        assert_eq!(
            scratch.text_box.rect(),
            scratch.text_rect(),
            "and the text fits"
        );
    }

    #[test]
    fn a_scratch_too_narrow_for_its_gutter_still_keeps_half_for_the_text() {
        let mut scratch =
            Scratch::new(Rect::new(0, 0, Area::new(6, 3)), &Config::default()).unwrap();
        scratch.open_buffer(Buffer::from("abc"));

        assert_eq!(scratch.gutter.width(), 3, "half of 6, not the 4 it wants");
        assert_eq!(scratch.text_rect().area.cols, 3);
        assert!(scratch.cursor_position().is_some());
    }

    #[test]
    fn the_text_is_at_most_the_max_text_width_and_centered_in_a_wider_scratch() {
        let scratch = scratch("abc", 200, 3);

        assert_eq!(
            scratch.text_rect().area.cols,
            MAX_TEXT_WIDTH,
            "the text stops at the max text width"
        );
        assert_eq!(
            scratch.block().col,
            (scratch.rect.area.cols - scratch.gutter.width() - MAX_TEXT_WIDTH) / 2,
            "the gutter and the text share the space around them evenly"
        );
    }
}
