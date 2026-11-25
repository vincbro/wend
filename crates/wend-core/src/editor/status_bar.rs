use crate::{
    frame::{Cell, CellPosition, Content, Grapheme, Rect, Style},
    layout::{Canvas, Edge, Element, Layout, Size},
};

/// The bottom row of the editor, which shows the text it was last updated with, or a message
/// instead of it while there is one.
#[derive(Debug)]
pub(super) struct StatusBar {
    text: String,
    message: String,
}

impl StatusBar {
    pub(super) fn new() -> Self {
        Self {
            text: String::new(),
            message: String::new(),
        }
    }

    /// Makes `content` the text the row shows.
    pub(super) fn update(&mut self, content: &str) {
        self.text.clear();
        self.text.push_str(content);
    }

    /// What the row shows in place of the text, none while it shows the text.
    pub(super) fn message(&self) -> Option<&str> {
        Some(self.message.as_str()).filter(|message| !message.is_empty())
    }

    /// Shows `message` in place of the text, until the next `clear_message`.
    pub(super) fn show(&mut self, message: &str) {
        self.message.clear();
        self.message.push_str(message);
    }

    pub(super) fn clear_message(&mut self) {
        self.message.clear();
    }

    /// What the row says: the message when there is one.
    fn shown(&self) -> &str {
        if self.message.is_empty() {
            &self.text
        } else {
            &self.message
        }
    }
}

impl Element for StatusBar {
    fn layout(&self) -> Layout {
        Layout::Dock {
            edge: Edge::Bottom,
            size: Size::Fixed(1),
        }
    }

    fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
        // A message is drawn as an error, the mode is plain text
        let content = if self.message.is_empty() {
            Content::Plain
        } else {
            Content::Error
        };
        // Text wider than the row is cut off at its edge
        for (c, col) in self
            .shown()
            .chars()
            .zip(rect.col..rect.col + rect.area.cols)
        {
            let cell = Cell {
                grapheme: Grapheme::from_char(c),
                style: Style {
                    content,
                    ..Style::default()
                },
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
        let mut status_bar = StatusBar::new();
        status_bar.update("wend");
        let mut frame = Frame::new(Area::new(cols, rows), None);
        let mut canvas = Canvas::root(&mut frame);
        let all = Rect::new(0, 0, Area::new(cols, rows));
        canvas.child(&mut status_bar, all);
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
    fn a_message_takes_the_place_of_the_text_until_it_is_cleared() {
        let mut status_bar = StatusBar::new();
        status_bar.update("nor");
        status_bar.show("unknown action `x`");
        assert_eq!(status_bar.shown(), "unknown action `x`");

        status_bar.clear_message();

        assert_eq!(status_bar.shown(), "nor", "the text is back");
    }

    #[test]
    fn no_room_is_no_text() {
        assert!(drawn(5, 0).iter().next().is_none(), "no rows, no cells");
        assert!(drawn(0, 3).iter().next().is_none(), "no columns, no cells");
    }
}
