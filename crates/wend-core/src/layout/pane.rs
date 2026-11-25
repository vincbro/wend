use crate::{
    frame::{Area, Cell, CellPosition, Content, Grapheme, Rect, Style},
    layout::{Canvas, Element, Layout},
};

/// Holds another element on a blank background inside a border, wherever `layout` puts it.
///
/// The blank background is what stops the cells below a floating pane from showing through
/// it. The element inside is given the space within the border and places itself in that
/// the way it would anywhere else.
#[derive(Debug, Clone)]
pub struct Pane<Inner> {
    layout: Layout,
    inner: Inner,
}

impl<Inner: Element> Pane<Inner> {
    /// How many cells the border takes on each side, so a pane around `n` rows is `n` plus twice
    /// this tall.
    pub const BORDER: u16 = 1;

    #[must_use]
    pub fn new(layout: Layout, inner: Inner) -> Self {
        Self { layout, inner }
    }

    #[must_use]
    pub fn inner(&self) -> &Inner {
        &self.inner
    }

    #[must_use]
    pub fn inner_mut(&mut self) -> &mut Inner {
        &mut self.inner
    }
}

impl<Inner: Element> Element for Pane<Inner> {
    fn layout(&self) -> Layout {
        self.layout
    }

    fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
        for row in 0..rect.area.rows {
            for col in 0..rect.area.cols {
                let pos = CellPosition::new(rect.col + col, rect.row + row);
                canvas.write(pos, cell(border_glyph(col, row, rect.area)));
            }
        }
        canvas.child(&mut self.inner, rect.inset(Self::BORDER));
    }
}

/// Returns the line drawn at `col` and `row` of a pane of `area`, none inside the border.
fn border_glyph(col: u16, row: u16, area: Area) -> Option<char> {
    let (left, right) = (col == 0, col + 1 == area.cols);
    let (top, bottom) = (row == 0, row + 1 == area.rows);
    match (top, bottom, left, right) {
        (true, _, true, _) => Some('┌'),
        (true, _, _, true) => Some('┐'),
        (_, true, true, _) => Some('└'),
        (_, true, _, true) => Some('┘'),
        (true, ..) | (_, true, ..) => Some('─'),
        (_, _, true, _) | (_, _, _, true) => Some('│'),
        _ => None,
    }
}

fn cell(glyph: Option<char>) -> Cell {
    let Some(glyph) = glyph else {
        return Cell {
            grapheme: Grapheme::blank(),
            ..Cell::default()
        };
    };
    Cell {
        grapheme: Grapheme::from_char(glyph),
        style: Style {
            content: Content::Border,
            ..Style::default()
        },
        ..Cell::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        frame::Frame,
        layout::{Fit, Size},
    };

    /// Writes a letter on the first row of its rect.
    struct Letter;

    impl Element for Letter {
        fn layout(&self) -> Layout {
            Layout::FILL
        }

        fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
            let cell = Cell {
                grapheme: Grapheme::from_char('x'),
                ..Cell::default()
            };
            canvas.write(CellPosition::new(rect.col, rect.row), cell);
        }
    }

    fn text(frame: &Frame, row: u16) -> String {
        frame
            .row(row)
            .iter()
            .map(|cell| cell.grapheme.as_str_sanitize())
            .collect()
    }

    fn floating(cols: u16, rows: u16) -> Pane<Letter> {
        Pane::new(
            Layout::Float(Fit::centered(Size::Fixed(cols), Size::Fixed(rows))),
            Letter,
        )
    }

    #[test]
    fn a_pane_draws_its_border_around_what_it_holds() {
        let mut frame = Frame::new(Area::new(6, 4), None);
        let all = Rect::new(0, 0, Area::new(6, 4));

        Canvas::root(&mut frame).child(&mut floating(4, 3), all);

        assert_eq!(
            [text(&frame, 0), text(&frame, 1), text(&frame, 2)],
            [" ┌──┐ ", " │x │ ", " └──┘ "],
            "the element is inside the border"
        );
        assert_eq!(text(&frame, 3), "      ", "nothing outside the pane");
    }

    #[test]
    fn a_pane_blanks_what_is_under_it() {
        let mut frame = Frame::new(Area::new(6, 3), None);
        let all = Rect::new(0, 0, Area::new(6, 3));
        let mut canvas = Canvas::root(&mut frame);
        let mut under = Pane::new(Layout::FILL, Letter);

        canvas.child(&mut under, all);
        canvas.child(&mut floating(4, 3), all);

        assert_eq!(
            text(&frame, 1),
            "││x ││",
            "the float covers the x under it, and the pane below shows only at the edges"
        );
    }

    #[test]
    fn a_pane_takes_the_cursor_under_it_away() {
        let mut frame = Frame::new(Area::new(6, 4), None);
        let all = Rect::new(0, 0, Area::new(6, 4));
        let mut canvas = Canvas::root(&mut frame);
        let mut cursor = Pane::new(Layout::FILL, Letter);

        canvas.child(&mut cursor, all);
        canvas.set_cursor(CellPosition::new(2, 1));
        canvas.child(&mut floating(4, 3), all);

        assert_eq!(frame.cursor(), None, "the pane covers the cursor's cell");
    }

    #[test]
    fn a_pane_with_no_room_draws_nothing() {
        let mut frame = Frame::new(Area::new(3, 3), None);
        let all = Rect::new(0, 0, Area::new(3, 3));

        Canvas::root(&mut frame).child(&mut floating(0, 2), all);

        assert!(
            (0..3).all(|row| text(&frame, row) == "   "),
            "no cells, no border"
        );
    }
}
