use std::{fmt::Debug, mem};

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
        // Go through the row so a col past the edge can't read the next row
        let slot = self.layout.row(row)?.slots.get(col).copied()?;
        self.layout.glyph(slot)
    }

    /// Puts the cursor on the grapheme starting at `byte`, or on the end of buffer slot.
    fn set_cursor_at(&mut self, byte: usize) {
        let rope = &self.buffer.rope;
        let byte = byte.min(rope.byte_len());
        // The end of buffer slot is one byte wide, see `Layout::rebuild`
        let len = rope
            .byte_slice(byte..)
            .graphemes()
            .next()
            .map_or(1, |grapheme| grapheme.len());
        self.cursor = Cursor::new(byte, byte.saturating_add(len));
    }

    fn last_visible_glyph(&self) -> Option<Glyph> {
        let rows: Vec<_> = self.layout.rows().collect();
        rows.iter()
            .rev()
            .flat_map(|row| row.slots.iter().rev())
            .find_map(|slot| self.layout.glyph(*slot))
    }

    /// Makes the column the cursor is on the one up and down aim for.
    fn sync_desired_col(&mut self) {
        if let Some((col, _)) = self.locate_cursor() {
            self.desired_col = col;
        }
    }

    // Finds the next glyph from a given slot
    fn next_glyph(&self, col: usize, row: usize) -> Option<Glyph> {
        let base_glyph = self.glyph(col, row);
        let next_col = col.saturating_add(1);
        let row = self.layout.row(row)?;

        for slot in row.slots.get(next_col..row.meta.len).unwrap_or_default() {
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
        let row = self.layout.row(row)?;
        let prev_col = col.saturating_add(1).min(row.slots.len());

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

    /// Deletes the grapheme before the cursor, joins lines when at the start of one.
    pub fn delete(&mut self) {
        // Done on the buffer, not the layout, so it works across wrapped rows and lines
        let to_byte = self.cursor.from_byte;
        let Some(len) = self
            .buffer
            .rope
            .byte_slice(..to_byte)
            .graphemes()
            .next_back()
            .map(|grapheme| grapheme.len())
        else {
            return;
        };
        let from_byte = to_byte.saturating_sub(len);
        self.buffer.delete(from_byte, to_byte);
        self.set_cursor_at(from_byte);
        self.ensure_cursor_visible();
        self.sync_desired_col();
    }

    /// Opens a new line below the cursor's line and puts the cursor on it.
    pub fn new_line(&mut self) {
        let rope = &self.buffer.rope;
        let line = rope.line_of_byte(self.cursor.from_byte);
        // The end of the line's text, before its line break
        let line_end = if line < rope.line_len() {
            rope.byte_of_line(line)
                .saturating_add(rope.line(line).byte_len())
        } else {
            rope.byte_len()
        };

        self.buffer.write(line_end, "\n");
        self.set_cursor_at(line_end.saturating_add(1));
        self.ensure_cursor_visible();
        self.sync_desired_col();
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
                // Scanning backwards lands on a glyph's last tail, sync to where it starts
                self.sync_desired_col();
                return;
            }
        }

        if let Some(prev_row) = self.layout.row(row_idx.saturating_sub(1))
            && row.meta.line == prev_row.meta.line
        {
            // len is a count, the last slot of the row is one before it
            self.desired_col = prev_row.meta.len.saturating_sub(1);
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

            // Our line wraps into more rows than fit on screen and we scroll by whole
            // lines, so the view can't reach the cursor, bring the cursor to the view
            if self.locate_cursor().is_none()
                && let Some(glyph) = self.last_visible_glyph()
            {
                self.cursor.set_glyph(glyph);
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
        // The old cursor and scroll are byte and line offsets into the old buffer
        self.scroll_offset = 0;
        self.desired_col = 0;
        self.set_cursor_at(0);
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
                    // The end of buffer slot points past the rope, it has nothing to show
                    let grapheme = if glyph.from_byte >= self.buffer.rope.byte_len() {
                        Grapheme::from_char(' ')
                    } else {
                        self.buffer.get_grapheme(glyph.as_range())
                    };

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
