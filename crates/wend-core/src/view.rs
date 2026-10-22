use crate::{
    area::Rect,
    buffer::{Buffer, Cursor},
    frame::{Cell, Frame},
    layout::Layout,
};

#[derive(Debug, Clone)]
pub struct View {
    pub buffer: Option<Buffer>,
    pub scroll_offset: usize,
    pub rect: Rect,
    pub layout: Layout,
}

impl View {
    pub fn up(&mut self) {}

    pub fn down(&mut self) {
        // If we don't have a buffer we don't have anything to render
        let Some(buffer) = &mut self.buffer else {
            return;
        };

        let mut layout = Layout::new();
        layout.model(
            buffer,
            usize::from(self.rect.area.cols),
            usize::from(self.rect.area.rows),
            self.scroll_offset,
        );

        for (row_idx, row) in layout.lines().enumerate() {
            for (slot_idx, slot) in row.slots.iter().copied().enumerate() {
                if let Some(glyph) = layout.glyph(slot)
                    && buffer.cursor.overlaps(glyph.as_range())
                    && let Some(new_glyph) = layout
                        .slot(layout.slot_index(row_idx.saturating_add(1), slot_idx))
                        .and_then(|slot| layout.glyph(slot))
                {
                    buffer.cursor = Cursor::new(new_glyph.from_byte, new_glyph.to_byte);
                    return;
                }
            }
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

        for (row, line) in self.layout.lines().enumerate() {
            let row = u16::try_from(row)
                .unwrap_or(u16::MAX)
                .saturating_add(self.rect.row);

            for (col, slot) in line.slots.iter().copied().enumerate() {
                let col = u16::try_from(col)
                    .unwrap_or(u16::MAX)
                    .saturating_add(self.rect.col);

                let cell = if let Some(glyph) = self.layout.glyph(slot) {
                    let content = buffer.rope.byte_slice(glyph.as_range());

                    Cell {
                        content: content.chars().next().unwrap_or('?'),
                        cursor: buffer.cursor.overlaps(glyph.as_range()),
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
