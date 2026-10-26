use std::{fmt::Debug, mem, ptr::read};

use crate::{
    area::Rect,
    buffer::Buffer,
    frame::{Cell, Frame},
    grapheme::Grapheme,
    view::layout::Glyph,
};

pub mod layout;
use layout::{Layout, Slot};
pub mod cursor;
use cursor::Cursor;

#[derive(Debug, Clone)]
pub struct View {
    buffer: Buffer,
    rect: Rect,
    layout: Layout,
    cursor: Cursor,

    scroll_offset: usize,
    scroll_marign: usize,
    desired_col: usize,
}

impl View {
    pub fn new(rect: Rect) -> Self {
        Self {
            buffer: Buffer::new(),
            rect,
            layout: Layout::new(rect.area, 0),
            cursor: Cursor::new(0, 1),

            scroll_offset: 0,
            scroll_marign: 10,
            desired_col: 0,
        }
    }

    /// Finds the cursor, scrolling first so it is always on screen.
    pub fn get_cursor(&mut self) -> (usize, usize) {
        self.ensure_cursor_visible();
        self.locate_cursor()
            .expect("The cursor is always visible after ensure_cursor_visible")
    }

    /// Looks up the cursor in the current layout, none if it is off screen.
    fn locate_cursor(&self) -> Option<(usize, usize)> {
        let byte = self.cursor.from_byte;
        for (row_idx, row) in self.layout.rows().enumerate() {
            if !(row.meta.from_byte..row.meta.to_byte).contains(&byte) {
                continue;
            }

            // The glyph the cursor sits on, works for both a block and an empty caret
            let Some(col_idx) = row.slots.iter().copied().position(|slot| {
                self.layout
                    .glyph(slot)
                    .is_some_and(|glyph| glyph.as_range().contains(&byte))
            }) else {
                continue;
            };

            return Some((col_idx, row_idx));
        }
        None
    }

    fn glyph(&self, col: usize, row: usize) -> Option<Glyph> {
        let idx = self.layout.slot_index(col, row);
        let slot = self.layout.slot(idx)?;
        self.layout.glyph(slot)
    }

    // Finds the next glyph from a given slot
    fn next_glyph(&self, col: usize, row: usize) -> Option<Glyph> {
        let base_glyph = self.glyph(col, row);
        let next_col = col.saturating_add(1);
        let row = self.layout.row(row)?;

        for slot in &row.slots[next_col..row.meta.len] {
            let Some(next_glyph) = self.layout.glyph(*slot) else {
                continue;
            };
            // This is a check so we don't land on the tail of the glyph we started on
            if base_glyph.is_some_and(|base_glyph| base_glyph == next_glyph) {
                continue;
            }
            return Some(next_glyph);
        }
        None
    }

    // Finds the previous glyph from a given slot
    fn prev_glyph(&self, col: usize, row: usize) -> Option<Glyph> {
        if col == 0 {
            return None;
        }
        let base_glyph = self.glyph(col, row);
        let prev_col = col.saturating_add(1);
        let row = self.layout.row(row)?;

        for slot in row.slots[0..prev_col].iter().rev() {
            let Some(prev_glyph) = self.layout.glyph(*slot) else {
                continue;
            };
            // This is a check so we don't land on the tail of the glyph we started on
            if base_glyph.is_some_and(|base_glyph| base_glyph == prev_glyph) {
                continue;
            }
            return Some(prev_glyph);
        }
        None
    }

    pub fn insert(&mut self, key: char) {
        let mut buf = [0; 4];
        let utf8_str = key.encode_utf8(&mut buf);
        let byte_idx = self.cursor.from_byte;
        self.cursor.from_byte = self.cursor.from_byte.saturating_add(utf8_str.len());
        self.cursor.to_byte = self.cursor.to_byte.saturating_add(utf8_str.len());
        self.buffer.write(byte_idx, utf8_str);
        self.rebuild();
        let (col, _) = self.get_cursor();
        self.desired_col = col;
    }

    pub fn delete(&mut self) {
        let (col, row) = self.get_cursor();
        let Some(glyph) = self.prev_glyph(col, row) else {
            return;
        };
        self.buffer.delete(glyph.from_byte, glyph.to_byte);
        self.cursor.set_glyph(glyph);
        self.rebuild();
    }

    pub fn new_line(&mut self) {
        let (_, row) = self.get_cursor();

        let Some(glyph) = self.glyph(0, row.saturating_add(1)) else {
            return;
        };
        self.cursor.set_glyph(glyph);
        self.insert('\n');
        self.cursor.set_glyph(glyph);
    }

    pub fn up(&mut self) {
        let (_, row_idx) = self.get_cursor();

        if row_idx == 0 {
            return;
        }

        let prev_row = row_idx.saturating_sub(1);
        let glyph = self
            .glyph(self.desired_col, prev_row)
            .or_else(|| self.prev_glyph(self.desired_col, prev_row))
            .or_else(|| self.next_glyph(self.desired_col, prev_row));

        if let Some(glyph) = glyph {
            self.cursor.set_glyph(glyph);
            self.ensure_cursor_visible();
        }
    }

    pub fn down(&mut self) {
        let (_, row_idx) = self.get_cursor();

        let next_row = row_idx.saturating_add(1);
        let glyph = self
            .glyph(self.desired_col, next_row)
            .or_else(|| self.prev_glyph(self.desired_col, next_row))
            .or_else(|| self.next_glyph(self.desired_col, next_row));

        if let Some(glyph) = glyph {
            self.cursor.set_glyph(glyph);
            self.ensure_cursor_visible();
        }
    }

    pub fn right(&mut self) {
        let (col_idx, row_idx) = self.get_cursor();

        let Some(row) = self.layout.row(row_idx) else {
            return;
        };
        for i in col_idx.saturating_add(1)..row.meta.len {
            if let Some(glyph) = self.layout.glyph(row.slots[i])
                && !self.cursor.is_glyph(glyph)
            {
                self.cursor.set_glyph(glyph);
                self.desired_col = i;
                self.ensure_cursor_visible();
                return;
            }
        }

        if let Some(next_row) = self.layout.row(row_idx.saturating_add(1))
            && row.meta.line == next_row.meta.line
        {
            self.desired_col = 0;
            self.down();
        }
    }

    pub fn left(&mut self) {
        let (col_idx, row_idx) = self.get_cursor();
        // If we don't have a buffer we don't have anything to render
        let Some(row) = self.layout.row(row_idx) else {
            return;
        };
        for i in (0..col_idx).rev() {
            if let Some(glyph) = self.layout.glyph(row.slots[i])
                && !self.cursor.is_glyph(glyph)
            {
                self.cursor.set_glyph(glyph);
                self.ensure_cursor_visible();
                return;
            }
        }

        if let Some(prev_row) = self.layout.row(row_idx.saturating_sub(1))
            && row.meta.line == prev_row.meta.line
        {
            self.desired_col = prev_row.meta.len;
            self.up();
        }
    }

    fn ensure_cursor_visible(&mut self) {
        let height = self.rect.area.height();
        let margin = self.scroll_marign.min(height.saturating_sub(1) / 2);

        // The buffer might have changed since the last rebuild
        self.rebuild();

        if self.locate_cursor().is_none() {
            let line = self.buffer.rope.line_of_byte(self.cursor.from_byte);
            self.scroll_offset = line.saturating_sub(margin);
            self.rebuild();

            // Lines above us wrapped into more rows than the margin, put our line on top
            if self.locate_cursor().is_none() {
                self.scroll_offset = line;
                self.rebuild();
            }
        }

        let Some((_, row)) = self.locate_cursor() else {
            return;
        };

        // Only ever scroll one way per call, a tall wrapped line could otherwise make
        // us bounce between the top and bottom margin forever
        let scroll_up = row < margin;
        let scroll_down = row + margin >= height;

        while let Some((_, row)) = self.locate_cursor() {
            let prev_offset = self.scroll_offset;
            if scroll_up && row < margin && self.scroll_offset > 0 {
                self.scroll_offset -= 1;
            } else if scroll_down && row + margin >= height {
                self.scroll_offset += 1;
            } else {
                break;
            }
            self.rebuild();

            // A tall wrapped line pushed the cursor off screen, undo and settle
            if self.locate_cursor().is_none() {
                self.scroll_offset = prev_offset;
                self.rebuild();
                break;
            }
        }
    }

    pub fn rebuild(&mut self) {
        self.layout
            .rebuild(&self.buffer, self.rect.area, self.scroll_offset);
    }

    pub fn resize(&mut self, rect: Rect) {
        self.rect = rect;
        self.rebuild();
    }

    pub fn open_buffer(&mut self, mut new_buffer: Buffer) -> Buffer {
        mem::swap(&mut new_buffer, &mut self.buffer);
        self.rebuild();
        new_buffer
    }

    pub fn render(&mut self, frame: &mut Frame) {
        self.rebuild();
        for (row, line) in self.layout.rows().enumerate() {
            let row = u16::try_from(row)
                .unwrap_or(u16::MAX)
                .saturating_add(self.rect.row);

            for (col, slot) in line.slots.iter().copied().enumerate() {
                let col = u16::try_from(col)
                    .unwrap_or(u16::MAX)
                    .saturating_add(self.rect.col);

                let cell = if let Some(glyph) = self.layout.glyph(slot) {
                    let grapheme = self.buffer.get_grapheme(glyph.as_range());

                    Cell {
                        grapheme,
                        cursor: self.cursor.overlaps(glyph.as_range()),
                        z_index: 0,
                    }
                } else if let Slot::Misc(char) = slot {
                    Cell {
                        grapheme: Grapheme::from_char(char),
                        cursor: false,
                        z_index: 0,
                    }
                } else {
                    Cell {
                        grapheme: Grapheme::from_char(' '),
                        cursor: false,
                        z_index: 0,
                    }
                };
                frame.write(col, row, cell);
            }
        }
    }
}
