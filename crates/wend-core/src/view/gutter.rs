use crate::{
    frame::{self, Cell, CellPosition, Frame, Grapheme, Rect},
    syntax::Highlight,
};

/// The fewest digits the gutter makes room for, so the text only moves when a file passes 999
/// lines.
const MIN_DIGITS: usize = 3;

/// Blank columns between the numbers and the text.
const PADDING: u16 = 1;

/// Where the cursor is, which every other row's number is counted from.
#[derive(Debug, Clone, Copy)]
pub(super) struct Head {
    /// The screen row the cursor is on.
    pub row: usize,
    /// The line the cursor is on, counted from 0.
    pub line: usize,
}

/// The columns at the left of a view that hold the line numbers.
///
/// It is as wide as the largest number it has to show, which is a line number or a row
/// distance, but takes at most half of the view so the text always has room. A gutter is
/// made for one size of view and buffer, so the view builds a new one when either changes.
#[derive(Debug, Clone, Copy)]
pub(super) struct Gutter {
    width: u16,
}

impl Gutter {
    /// Makes the gutter of a view `view_cols` wide that has to show numbers up to `largest`.
    #[must_use]
    pub(super) fn new(largest: usize, view_cols: u16) -> Self {
        let digits = Digits::new(largest).len().max(MIN_DIGITS);
        let digits = u16::try_from(digits).expect("a usize has at most 20 digits, which fit a u16");
        Self {
            width: (digits + PADDING).min(view_cols / 2),
        }
    }

    /// Splits the view's `rect` into the gutter and the part left for the text.
    #[must_use]
    pub(super) fn split(self, rect: Rect) -> (Rect, Rect) {
        rect.split_left(self.width)
    }

    /// Writes the numbers of the first `rows_with_text` rows into the gutter part of `rect`,
    /// and blanks the rest.
    ///
    /// The cursor's row shows its line number, counted from 1. Every other row shows how many
    /// rows it is from the cursor's, which is how many times a vertical motion takes the
    /// cursor there, wrapped rows included.
    ///
    /// Every cell of the gutter is written, because the frame is reused and still holds the
    /// last draw. Nothing is numbered without a `head`.
    pub(super) fn render(
        self,
        frame: &mut Frame,
        rect: Rect,
        rows_with_text: usize,
        head: Option<Head>,
    ) {
        let (rect, _) = self.split(rect);
        let numbers = rect.pad_right(PADDING);
        for row in 0..rect.area.height() {
            blank_row(frame, rect, row);
            let Some(head) = head.filter(|_| row < rows_with_text) else {
                continue;
            };
            write_number(frame, numbers, row, number(row, head), row == head.row);
        }
    }
}

fn number(row: usize, head: Head) -> usize {
    if row == head.row {
        head.line + 1
    } else {
        row.abs_diff(head.row)
    }
}

fn blank_row(frame: &mut Frame, rect: Rect, row: usize) {
    let blank = Cell {
        grapheme: Grapheme::blank(),
        ..Cell::default()
    };
    let row = rect.row + frame::terminal_coordinate(row);
    for col in 0..rect.area.cols {
        frame.write(CellPosition::new(rect.col + col, row), blank);
    }
}

/// Writes `number` ending at the right edge of `rect`. A number that doesn't fit is left out,
/// because its last digits alone would read as another number.
fn write_number(frame: &mut Frame, rect: Rect, row: usize, number: usize, current: bool) {
    let digits = Digits::new(number);
    if digits.len() > rect.area.width() {
        return;
    }
    let highlight = if current {
        Highlight::Keyword
    } else {
        Highlight::Comment
    };
    let row = rect.row + frame::terminal_coordinate(row);
    let right_edge = rect.col + rect.area.cols;
    for (offset, digit) in digits.rev().enumerate() {
        let col = right_edge - frame::terminal_coordinate(offset) - 1;
        let cell = Cell {
            grapheme: Grapheme::from_char(digit),
            highlight: Some(highlight),
            ..Cell::default()
        };
        frame.write(CellPosition::new(col, row), cell);
    }
}

/// The digits of a number, from either end.
#[derive(Clone)]
struct Digits {
    /// The digits not taken yet, as a number that may have leading zeros.
    rest: usize,
    /// The value of the place the front digit sits in.
    place_value: usize,
    remaining: usize,
}

impl Digits {
    fn new(number: usize) -> Self {
        let exponent = number.checked_ilog10().unwrap_or(0);
        Self {
            rest: number,
            place_value: 10usize.pow(exponent),
            remaining: exponent as usize + 1,
        }
    }
}

fn digit_char(digit: usize) -> Option<char> {
    char::from_digit(u32::try_from(digit).ok()?, 10)
}

impl Iterator for Digits {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        if self.remaining == 0 {
            return None;
        }
        let digit = self.rest / self.place_value;
        self.rest %= self.place_value;
        self.place_value = (self.place_value / 10).max(1);
        self.remaining -= 1;
        digit_char(digit)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl DoubleEndedIterator for Digits {
    fn next_back(&mut self) -> Option<char> {
        if self.remaining == 0 {
            return None;
        }
        let digit = self.rest % 10;
        self.rest /= 10;
        self.place_value = (self.place_value / 10).max(1);
        self.remaining -= 1;
        digit_char(digit)
    }
}

impl ExactSizeIterator for Digits {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Area;

    const HEAD: Head = Head { row: 5, line: 41 };

    /// How many columns the gutter takes.
    fn width(gutter: Gutter) -> u16 {
        gutter
            .split(Rect::new(0, 0, Area::new(u16::MAX, 1)))
            .0
            .area
            .cols
    }

    /// A gutter four columns wide, in a view of eight.
    fn gutter() -> Gutter {
        Gutter::new(0, 8)
    }

    /// What the first four columns of a row of the frame show.
    fn row_text(frame: &Frame, row: u16) -> String {
        frame.row(row)[..4]
            .iter()
            .map(|cell| cell.grapheme.as_str_sanitize())
            .collect()
    }

    fn rendered(rows: u16, rows_with_text: usize, head: Option<Head>) -> Frame {
        let area = Area::new(8, rows);
        let mut frame = Frame::new(area, None);
        gutter().render(&mut frame, Rect::new(0, 0, area), rows_with_text, head);
        frame
    }

    #[test]
    fn the_gutter_grows_with_the_largest_number() {
        let width = |largest| width(Gutter::new(largest, 100));

        assert_eq!(width(0), 4, "three digits and the padding at the least");
        assert_eq!(width(999), 4, "still three digits");
        assert_eq!(width(1_000), 5, "a fourth digit");
        assert_eq!(width(99_999), 6, "five digits");
    }

    #[test]
    fn the_gutter_takes_at_most_half_of_the_view() {
        assert_eq!(
            width(Gutter::new(1_000, 6)),
            3,
            "half of 6, not the 5 it wants"
        );
    }

    #[test]
    fn the_gutter_splits_off_the_left_of_a_rect() {
        let (gutter, text) = gutter().split(Rect::new(2, 1, Area::new(8, 3)));

        assert_eq!(
            (gutter.col, gutter.area.cols),
            (2, 4),
            "the gutter comes first"
        );
        assert_eq!((text.col, text.area.cols), (6, 4), "the text gets the rest");
    }

    #[test]
    fn the_cursor_row_shows_its_line_and_the_others_their_distance() {
        let at = |row| number(row, HEAD);

        assert_eq!(at(5), 42, "the cursor's line, counted from 1");
        assert_eq!(at(4), 1, "one row above");
        assert_eq!(
            at(0),
            5,
            "the distance does not depend on where the cursor is"
        );
        assert_eq!(at(6), 1, "one row below");
        assert_eq!(at(11), 6, "six rows below");
    }

    #[test]
    fn rows_are_numbered_right_aligned_with_the_padding_left_blank() {
        let frame = rendered(9, 8, Some(HEAD));

        assert_eq!(row_text(&frame, 0), "  5 ", "five rows above the cursor");
        assert_eq!(row_text(&frame, 4), "  1 ", "one row above the cursor");
        assert_eq!(row_text(&frame, 5), " 42 ", "the cursor's line");
        assert_eq!(row_text(&frame, 7), "  2 ", "two rows below the cursor");
    }

    #[test]
    fn rows_without_text_are_blank() {
        let frame = rendered(9, 8, Some(HEAD));

        assert_eq!(row_text(&frame, 8), "    ", "the file ends before this row");
    }

    #[test]
    fn nothing_is_numbered_without_a_cursor_on_screen() {
        let frame = rendered(3, 3, None);

        assert!(
            (0..3).all(|row| row_text(&frame, row) == "    "),
            "every row is blank"
        );
    }

    #[test]
    fn a_shorter_number_leaves_none_of_the_last_draw_behind() {
        let area = Area::new(8, 1);
        let mut frame = Frame::new(area, None);
        let rect = Rect::new(0, 0, area);

        gutter().render(&mut frame, rect, 1, Some(Head { row: 0, line: 99 }));
        assert_eq!(row_text(&frame, 0), "100 ", "line 100 first");
        gutter().render(&mut frame, rect, 1, Some(Head { row: 0, line: 4 }));

        assert_eq!(row_text(&frame, 0), "  5 ", "no 1 or 0 left of the 5");
    }

    #[test]
    fn a_number_that_does_not_fit_is_left_out() {
        let area = Area::new(6, 1);
        let mut frame = Frame::new(area, None);

        Gutter::new(0, 6).render(
            &mut frame,
            Rect::new(0, 0, area),
            1,
            Some(Head { row: 0, line: 122 }),
        );

        assert_eq!(
            &row_text(&frame, 0)[..3],
            "   ",
            "23 would read as another line, so nothing is shown"
        );
    }

    #[test]
    fn digits_come_from_either_end() {
        assert_eq!(Digits::new(0).collect::<String>(), "0", "zero is a digit");
        assert_eq!(Digits::new(907).collect::<String>(), "907", "front first");
        assert_eq!(
            Digits::new(907).rev().collect::<String>(),
            "709",
            "back first"
        );
    }
}
