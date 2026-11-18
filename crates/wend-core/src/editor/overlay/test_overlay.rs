use crate::{
    buffer::Buffer,
    config::Config,
    frame::{Area, Rect},
    layout::{Canvas, Edge, Element, Fit, Layout, Pane, Size},
    view::{Result, View},
};

/// What the view in the left window shows.
const LEFT: &str = "the left window\nwith its own line numbers";

/// What the view in the right window shows.
const RIGHT: &str = "the right window\nnext to the left one";

/// Two windows side by side in a float, to try arranging windows in an overlay until the
/// file picker needs it.
#[derive(Debug)]
pub(in crate::editor) struct TestOverlay {
    left: Pane<View>,
    right: Pane<View>,
}

impl TestOverlay {
    pub(in crate::editor) fn new(config: &Config) -> Result<Self> {
        let left = Layout::Dock {
            edge: Edge::Left,
            size: Size::Percent(40),
        };
        Ok(Self {
            left: Pane::new(left, view(LEFT, config)?),
            right: Pane::new(Layout::FILL, view(RIGHT, config)?),
        })
    }
}

impl Element for TestOverlay {
    fn layout(&self) -> Layout {
        Layout::Float(Fit::centered(Size::Percent(80), Size::Percent(60)))
    }

    /// The left window takes its part first, so the right one gets what is left.
    fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
        let free = canvas.child(&mut self.left, rect);
        canvas.child(&mut self.right, free);
    }
}

/// A view that shows `text`, as big as the rect it is drawn in.
fn view(text: &str, config: &Config) -> Result<View> {
    let mut view = View::new(Rect::new(0, 0, Area::new(0, 0)), config)?;
    // The view starts empty, so there is no buffer to keep
    drop(view.open_buffer(Buffer::from(text)));
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Frame;

    fn drawn(cols: u16, rows: u16) -> Frame {
        let mut frame = Frame::new(Area::new(cols, rows), None);
        let mut canvas = Canvas::root(&mut frame);
        let all = Rect::new(0, 0, Area::new(cols, rows));
        canvas.child(&mut TestOverlay::new(&Config::default()).unwrap(), all);
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
    fn the_windows_sit_side_by_side_on_the_same_rows() {
        let frame = drawn(100, 20);

        let row = (0..20)
            .map(|row| text(&frame, row))
            .find(|row| row.contains("the left window"))
            .expect("the left window is drawn");
        let left = row.find("the left window").unwrap();
        let right = row
            .find("the right window")
            .expect("the right window is on the same row");
        assert!(left < right, "the left window is left of the right one");
    }

    #[test]
    fn the_windows_float_in_the_middle_and_leave_the_edges_alone() {
        let frame = drawn(100, 20);

        assert_eq!(
            text(&frame, 0).trim(),
            "",
            "the first row is above the float"
        );
        assert_eq!(text(&frame, 19).trim(), "", "and the last row is below it");
    }
}
