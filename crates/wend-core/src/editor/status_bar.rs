use crate::{
    frame::{Cell, CellPosition, Grapheme, Rect},
    layout::{Canvas, Edge, Element, Layout, Size},
};

/// What the status bar says, until it has something to say.
const TEXT: &str = "wend";

/// The bottom row of the editor, which shows the same text whatever the editor is doing.
#[derive(Debug)]
pub(super) struct StatusBar;

impl Element for StatusBar {
    fn layout(&self) -> Layout {
        Layout::Dock {
            edge: Edge::Bottom,
            size: Size::Fixed(1),
        }
    }

    fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
        // Text wider than the row is cut off at its edge
        for (c, col) in TEXT.chars().zip(rect.col..rect.col + rect.area.cols) {
            let cell = Cell {
                grapheme: Grapheme::from_char(c),
                ..Cell::default()
            };
            canvas.write(CellPosition::new(col, rect.row), cell);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{Area, Frame};

    fn drawn(cols: u16, rows: u16) -> Frame {
        let mut frame = Frame::new(Area::new(cols, rows), None);
        let mut canvas = Canvas::root(&mut frame);
        let all = Rect::new(0, 0, Area::new(cols, rows));
        canvas.child(&mut StatusBar, all);
        frame
    }

    fn text(frame: &Frame, row: u16) -> String {
        frame
            .row(row)
            .iter()
            .map(|cell| cell.grapheme.as_str_sanitize())
            .collect()
    }

    #[test]
    fn the_text_is_on_the_last_row() {
        let frame = drawn(6, 3);

        assert_eq!(text(&frame, 2), "wend  ", "the bottom row");
        assert_eq!(text(&frame, 0).trim(), "", "and nothing above it");
    }

    #[test]
    fn text_wider_than_the_row_is_cut_off() {
        assert_eq!(text(&drawn(2, 1), 0), "we");
    }

    #[test]
    fn no_room_is_no_text() {
        assert!(drawn(5, 0).iter().next().is_none(), "no rows, no cells");
        assert!(drawn(0, 3).iter().next().is_none(), "no columns, no cells");
    }
}
