use crate::{
    area::Rect,
    buffer::Buffer,
    frame::{Cell, Frame},
    layout::{Layout, Slot},
};

#[derive(Debug, Clone)]
pub struct View {
    pub buffer: Option<Buffer>,
    pub scroll_offset: usize,
    pub rect: Rect,
    pub layout: Layout,
}

impl View {
    pub fn up(&mut self) {
        // If we don't have a buffer we don't have anything to render
        let Some(buffer) = &mut self.buffer else {
            return;
        };

        let width = usize::from(self.rect.area.cols);
        let lines = usize::from(self.rect.area.rows);
        if !self.layout.fits(width, lines, self.scroll_offset) {
            self.layout.model(buffer, width, lines, self.scroll_offset);
        }

        for (row_idx, row) in self.layout.rows().enumerate() {
            let Some(meta) = row.meta else {
                continue;
            };
            if !buffer.cursor.overlaps(meta.from_byte..meta.to_byte) {
                continue;
            }

            let Some(col) = row.slots.iter().copied().position(|slot| {
                if let Some(glyph) = self.layout.glyph(slot) {
                    buffer.cursor.overlaps(glyph.as_range())
                } else {
                    false
                }
            }) else {
                continue;
            };

            let Some(prev_row) = self.layout.row(row_idx.saturating_sub(1)) else {
                break;
            };

            for i in (0..=col).rev() {
                if let Some(glyph) = self.layout.glyph(prev_row.slots[i]) {
                    buffer.cursor.from_byte = glyph.from_byte;
                    buffer.cursor.to_byte = glyph.to_byte;
                    break;
                }
            }
            break;
        }
    }

    pub fn down(&mut self) {
        // If we don't have a buffer we don't have anything to render
        let Some(buffer) = &mut self.buffer else {
            return;
        };

        let width = usize::from(self.rect.area.cols);
        let lines = usize::from(self.rect.area.rows);
        if !self.layout.fits(width, lines, self.scroll_offset) {
            self.layout.model(buffer, width, lines, self.scroll_offset);
        }

        for (row_idx, row) in self.layout.rows().enumerate() {
            let Some(meta) = row.meta else {
                continue;
            };
            if !buffer.cursor.overlaps(meta.from_byte..meta.to_byte) {
                continue;
            }

            let Some(col) = row.slots.iter().copied().position(|slot| {
                if let Some(glyph) = self.layout.glyph(slot) {
                    buffer.cursor.overlaps(glyph.as_range())
                } else {
                    false
                }
            }) else {
                continue;
            };

            let Some(next_row) = self.layout.row(row_idx.saturating_add(1)) else {
                break;
            };

            for i in (0..=col).rev() {
                if let Some(glyph) = self.layout.glyph(next_row.slots[i]) {
                    buffer.cursor.from_byte = glyph.from_byte;
                    buffer.cursor.to_byte = glyph.to_byte;
                    break;
                }
            }
            break;
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        // If we don't have a buffer we don't have anything to render
        let Some(buffer) = &self.buffer else {
            return;
        };

        let width = usize::from(self.rect.area.cols);
        let lines = usize::from(self.rect.area.rows);

        if !self.layout.fits(width, lines, self.scroll_offset) {
            self.layout.model(buffer, width, lines, self.scroll_offset);
        }

        for (row, line) in self.layout.rows().enumerate() {
            let row = u16::try_from(row)
                .unwrap_or(u16::MAX)
                .saturating_add(self.rect.row);

            for (col, slot) in line.slots.iter().copied().enumerate() {
                let col = u16::try_from(col)
                    .unwrap_or(u16::MAX)
                    .saturating_add(self.rect.col);

                let cell = if let Some(glyph) = self.layout.glyph(slot) {
                    let content = buffer.rope.byte_slice(glyph.as_range());
                    let content = content.chars().next().unwrap_or('?');

                    let content = match content {
                        '\n' | '\t' | '\r' => ' ',
                        _ => content,
                    };

                    Cell {
                        content,
                        cursor: buffer.cursor.overlaps(glyph.as_range()),
                        z_index: 0,
                    }
                } else if let Slot::Misc(char) = slot {
                    Cell {
                        content: char,
                        cursor: false,
                        z_index: 0,
                    }
                } else {
                    Cell {
                        content: ' ',
                        cursor: false,
                        z_index: 0,
                    }
                };
                frame.write(col, row, cell);
            }
        }
    }
}
