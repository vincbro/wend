use std::{fs, io, path::Path};

use crate::{
    area::{Area, Rect},
    buffer::Buffer,
    frame::Frame,
    page::Page,
};

pub struct Editor {
    pub area: Area,
    pub buffers: Vec<Buffer>,
    pub page: Page,
}

impl Editor {
    pub fn new(area: Area) -> Self {
        Self {
            area,
            buffers: vec![],
            page: Page {
                buffer: None,
                scroll_offset: 0,
            },
        }
    }

    pub fn set_buffer(&mut self, buffer: Buffer) {
        if let Some(other_buffer) = self.page.buffer.take() {
            self.buffers.push(other_buffer);
        }
        self.page.buffer = Some(buffer);
    }

    pub fn open_file<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let path = path.as_ref();

        let buffer = if let Some(index) = self.buffers.iter().position(|buffer| buffer.file == path)
        {
            self.buffers.swap_remove(index)
        } else {
            Buffer::new(path)?
        };

        self.set_buffer(buffer);

        Ok(())
    }

    pub fn frame(&self) -> Frame {
        let mut frame = Frame::new(self.area);
        let page_rect = Rect::new(0, 0, self.area);
        self.page.render(page_rect, &mut frame);
        frame
    }
}
