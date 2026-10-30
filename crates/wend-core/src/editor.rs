use std::path::Path;

use crate::{
    buffer::{self, Buffer},
    frame::{Area, Frame, Rect},
    view::View,
};

/// The widest the view gets, wider terminals center it.
const MAX_VIEW_COLS: u16 = 100;

/// Returns where the view goes in a terminal of `area`.
fn view_rect(area: Area) -> Rect {
    let width = area.cols.min(MAX_VIEW_COLS);
    let left_col = (area.cols - width) / 2;
    Rect::new(left_col, 0, Area::new(width, area.rows))
}

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Normal,
    /// Works like normal, except motions extend the selection instead of moving it.
    Select,
    Insert,
}

/// Holds the whole editor: the terminal's area, the view and the mode.
///
/// `buffers` holds every open buffer except the one the view shows, so each open file
/// lives in exactly one place.
#[derive(Debug, Clone)]
pub struct Editor {
    area: Area,
    buffers: Vec<Buffer>,
    view: View,
    mode: Mode,
}

impl Editor {
    #[must_use]
    pub fn new(area: Area) -> Self {
        Self {
            area,
            buffers: vec![],
            view: View::new(view_rect(area)),
            mode: Mode::Normal,
        }
    }

    #[must_use]
    pub fn view(&self) -> &View {
        &self.view
    }

    pub fn view_mut(&mut self) -> &mut View {
        &mut self.view
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Shows the file at `path`, reusing its buffer if it's among the open buffers. The buffer
    /// shown before joins them.
    pub fn open_file(&mut self, path: &Path) -> buffer::Result<()> {
        let buffer = if let Some(index) = self
            .buffers
            .iter()
            .position(|buffer| buffer.path() == Some(path))
        {
            self.buffers.swap_remove(index)
        } else {
            Buffer::from_path(path)?
        };
        self.buffers.push(self.view.open_buffer(buffer));
        Ok(())
    }

    /// Builds a new frame of the terminal's size, with the cursor where the head is now.
    pub fn frame(&mut self) -> Frame {
        // Scroll to the cursor before rendering, so the frame shows where the cursor is
        let cursor = self.view.cursor_position();
        let mut frame = Frame::new(self.area, cursor);
        self.update_frame(&mut frame);
        frame
    }

    /// Redraws the view into `frame`, reusing its cells. `Frame::cursor` stays as it was, only
    /// `frame` gives a new one.
    ///
    /// # Panics
    ///
    /// If `frame` is smaller than the view reaches, as one from before a resize can be.
    pub fn update_frame(&mut self, frame: &mut Frame) {
        self.view.render(frame);
    }

    /// Fits the editor to a new terminal size, keeping the view capped and centered.
    pub fn resize(&mut self, area: Area) {
        self.area = area;
        self.view.resize(view_rect(area));
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    #[must_use]
    pub fn is_select(&self) -> bool {
        matches!(self.mode, Mode::Select)
    }

    #[must_use]
    pub fn is_insert(&self) -> bool {
        matches!(self.mode, Mode::Insert)
    }
}
