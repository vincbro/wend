use crate::{
    frame::{self, Cell, CellPosition, Frame, Grapheme, Rect},
    syntax::Highlight,
};

/// The fewest digits the gutter makes room for, so the text only moves when a file passes 999
/// lines.
const MIN_DIGITS: usize = 3;

/// Blank columns between the numbers and the text.
const PADDING: u16 = 2;

/// Where the cursor is, which every other row's number is counted from.
#[derive(Debug, Clone, Copy)]
pub(super) struct Head {
    /// The screen row the cursor is on.
    pub row: usize,
    /// The line the cursor is on, counted from 0.
    pub line: usize,
}

/// Returns how wide the gutter is when it has to fit numbers up to `largest`.
pub(super) fn width(largest: usize) -> u16 {
    let digits = digit_count(largest).max(MIN_DIGITS);
    let digits = u16::try_from(digits).expect("a usize has at most 20 digits, which fit a u16");
    digits + PADDING
}

/// Writes the numbers of the first `rows_with_text` rows into `rect`, and blanks the rest.
///
/// The cursor's row shows its line number, counted from 1. Every other row shows how many rows
/// it is from the cursor's, which is how many times a vertical motion takes the cursor there,
/// wrapped rows included.
///
/// Every cell of `rect` is written, because the frame is reused and still holds the last draw.
/// Nothing is numbered without a `head`.
pub(super) fn render(frame: &mut Frame, rect: Rect, rows_with_text: usize, head: Option<Head>) {
    let numbers = rect.pad_right(PADDING);
    for row in 0..rect.area.height() {
        blank_row(frame, rect, row);
        let Some(head) = head.filter(|_| row < rows_with_text) else {
            continue;
        };

        let current = row == head.row;
        let number = if current {
            head.line + 1
        } else {
            row.abs_diff(head.row)
        };
        write_number(frame, numbers, row, number, current);
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
    let digits = digits(number);
    if digits.len() > rect.area.width() {
        return;
    }
    let row = rect.row + frame::terminal_coordinate(row);
    let right_edge = rect.col + rect.area.cols;
    for (offset, digit) in digits.rev().enumerate() {
        let col = right_edge - frame::terminal_coordinate(offset) - 1;
        let cell = Cell {
            grapheme: Grapheme::from_char(digit),
            highlight: if current {
                Some(Highlight::Constant)
            } else {
                Some(Highlight::Comment)
            },
            ..Cell::default()
        };
        frame.write(CellPosition::new(col, row), cell);
    }
}

fn digit_count(number: usize) -> usize {
    number.checked_ilog10().map_or(1, |exp| exp as usize + 1)
}

/// The digits of a number, from either end.
#[derive(Clone)]
struct Digits {
    /// The digits not taken yet, as a number that may have leading zeros.
    remaining: usize,
    /// 10 to the power of the digits left, less one, and at least 1.
    divisor: usize,
    left: usize,
}

fn digits(number: usize) -> Digits {
    let exp = number.checked_ilog10().unwrap_or(0);
    Digits {
        remaining: number,
        divisor: 10usize.pow(exp),
        left: exp as usize + 1,
    }
}

fn digit_char(digit: usize) -> Option<char> {
    char::from_digit(u32::try_from(digit).ok()?, 10)
}

impl Iterator for Digits {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        if self.left == 0 {
            return None;
        }
        let digit = self.remaining / self.divisor;
        self.remaining %= self.divisor;
        self.divisor = (self.divisor / 10).max(1);
        self.left -= 1;
        digit_char(digit)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left, Some(self.left))
    }
}

impl DoubleEndedIterator for Digits {
    fn next_back(&mut self) -> Option<char> {
        if self.left == 0 {
            return None;
        }
        let digit = self.remaining % 10;
        self.remaining /= 10;
        self.divisor = (self.divisor / 10).max(1);
        self.left -= 1;
        digit_char(digit)
    }
}

impl ExactSizeIterator for Digits {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Area;

    const HEAD: Head = Head { row: 5, line: 41 };

    /// What a row of the frame shows, from its first column.
    fn row_text(frame: &Frame, row: u16) -> String {
        frame
            .row(row)
            .iter()
            .map(|cell| cell.grapheme.as_str_sanitize())
            .collect()
    }

    fn rendered(rows: u16, rows_with_text: usize, head: Option<Head>) -> Frame {
        let mut frame = Frame::new(Area::new(4, rows), None);
        render(
            &mut frame,
            Rect::new(0, 0, Area::new(4, rows)),
            rows_with_text,
            head,
        );
        frame
    }

    #[test]
    fn the_gutter_grows_with_the_largest_number() {
        assert_eq!(width(0), 4, "three digits and the padding at the least");
        assert_eq!(width(999), 4, "still three digits");
        assert_eq!(width(1_000), 5, "a fourth digit");
        assert_eq!(width(99_999), 6);
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
        assert_eq!(at(11), 6);
    }

    #[test]
    fn rows_are_numbered_right_aligned_with_the_padding_left_blank() {
        let frame = rendered(9, 8, Some(HEAD));

        assert_eq!(row_text(&frame, 0), "  5 ");
        assert_eq!(row_text(&frame, 4), "  1 ");
        assert_eq!(row_text(&frame, 5), " 42 ", "the cursor's line");
        assert_eq!(row_text(&frame, 7), "  2 ");
    }

    #[test]
    fn rows_without_text_are_blank() {
        let frame = rendered(9, 8, Some(HEAD));

        assert_eq!(row_text(&frame, 8), "    ", "the file ends before this row");
    }

    #[test]
    fn nothing_is_numbered_without_a_cursor_on_screen() {
        let frame = rendered(3, 3, None);

        assert!((0..3).all(|row| row_text(&frame, row) == "    "));
    }

    #[test]
    fn a_shorter_number_leaves_none_of_the_last_draw_behind() {
        let mut frame = Frame::new(Area::new(4, 1), None);
        let rect = Rect::new(0, 0, Area::new(4, 1));

        render(&mut frame, rect, 1, Some(Head { row: 0, line: 99 }));
        assert_eq!(row_text(&frame, 0), "100 ", "line 100 first");
        render(&mut frame, rect, 1, Some(Head { row: 0, line: 4 }));

        assert_eq!(row_text(&frame, 0), "  5 ", "no 1 or 0 left of the 5");
    }

    #[test]
    fn a_number_that_does_not_fit_is_left_out() {
        let mut frame = Frame::new(Area::new(3, 1), None);
        let rect = Rect::new(0, 0, Area::new(3, 1));

        render(&mut frame, rect, 1, Some(Head { row: 0, line: 122 }));

        assert_eq!(
            row_text(&frame, 0),
            "   ",
            "23 would read as another line, so nothing is shown"
        );
    }

    #[test]
    fn digits_come_from_either_end() {
        assert_eq!(digits(0).collect::<String>(), "0");
        assert_eq!(digits(907).collect::<String>(), "907");
        assert_eq!(digits(907).rev().collect::<String>(), "709");
    }
}
