use std::path::Path;

use crate::{
    area::{Area, Rect},
    buffer::{self, Buffer},
    frame::Frame,
    view::View,
};

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Normal,
    /// Normal, except motions extend the selection instead of moving it.
    Select,
    Insert,
}

/// The whole editor: the terminal's area, the view and the mode.
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
        let width = area.cols.min(100);
        let left_col = (area.cols - width) / 2;
        let view_area = Area::new(width, area.rows);
        Self {
            area,
            buffers: vec![],
            view: View::new(Rect::new(left_col, 0, view_area)),
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

    pub fn frame(&mut self) -> Frame {
        // Scroll to the cursor before rendering, so the frame shows where the cursor is
        let cursor = self.view.cursor_position();
        let mut frame = Frame::new(self.area, self.mode, cursor);
        self.update_frame(&mut frame);
        frame
    }

    pub fn update_frame(&mut self, frame: &mut Frame) {
        self.view.render(frame);
    }

    pub fn resize(&mut self, area: Area) {
        self.area = area;
        self.view.resize(Rect::new(0, 0, area));
    }

    pub fn switch_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    #[must_use]
    pub fn is_normal(&self) -> bool {
        matches!(self.mode, Mode::Normal)
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
