use std::{io, path::Path};

use crate::{
    area::{Area, Rect},
    buffer::Buffer,
    frame::Frame,
    view::View,
};

#[derive(Debug, Clone)]
pub struct Editor {
    pub area: Area,
    pub buffers: Vec<Buffer>,
    pub view: View,
}

impl Editor {
    #[must_use]
    pub fn new(area: Area) -> Self {
        Self {
            area,
            buffers: vec![],
            view: View::new(Rect::new(0, 0, area)),
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
        self.view.render(&mut frame);
        frame
    }
}
