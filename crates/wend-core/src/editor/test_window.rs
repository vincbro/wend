use crate::{
    buffer::Buffer,
    config::Config,
    frame::{Area, Rect},
    layout::{Fit, Layout, Pane, Size},
    view::View,
};

/// What the view in the test window shows.
const TEXT: &str = "a view inside a floating pane\nwith its own line numbers\nand a line that is long enough to wrap around the edge of the pane";

/// A floating pane in the middle of the editor with a view in it, to try out floats until
/// there is a real one.
pub(super) fn test_window(config: &Config) -> Pane<View> {
    let mut view = View::new(Rect::new(0, 0, Area::new(0, 0)), config);
    // The view starts empty, so there is no buffer to keep
    drop(view.open_buffer(Buffer::from(TEXT)));
    Pane::new(
        Layout::Float(Fit::centered(Size::Percent(60), Size::Percent(50))),
        view,
    )
}
