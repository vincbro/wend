use std::{io, path::Path};

use crate::{
    area::{Area, Rect, saturating_u16},
    buffer::Buffer,
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

#[derive(Debug, Clone)]
pub struct Editor {
    pub area: Area,
    pub buffers: Vec<Buffer>,
    pub view: View,
    pub mode: Mode,
}

impl Editor {
    #[must_use]
    pub fn new(area: Area) -> Self {
        let width = area.width().min(50);
        let overflow = saturating_u16(area.width().saturating_sub(width) / 2);
        let view = Area::new(saturating_u16(width), area.rows);
        Self {
            area,
            buffers: vec![],
            view: View::new(Rect::new(overflow, 0, view)),
            mode: Mode::Normal,
        }
    }

    pub fn open_file<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let path = path.as_ref();

        let buffer = if let Some(index) = self
            .buffers
            .iter()
            .position(|buffer| buffer.path.iter().any(|p| p == path))
        {
            self.buffers.swap_remove(index)
        } else {
            Buffer::from_path(path)?
        };
        self.buffers.push(self.view.open_buffer(buffer));
        Ok(())
    }

    pub fn frame(&mut self) -> Frame {
        let mut frame = Frame::new(self.area);
        // Scroll to the cursor before rendering, so the frame shows where the cursor is
        frame.cursor = self.view.cursor_position();
        self.view.render(&mut frame);
        frame.mode = self.mode;
        frame
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
