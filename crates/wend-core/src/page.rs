use crate::{
    area::Rect,
    buffer::Buffer,
    frame::{Cell, Frame},
};

pub struct Page {
    pub buffer: Option<Buffer>,
    pub scroll_offset: usize,
}

impl Page {
    pub fn render(&self, rect: Rect, frame: &mut Frame) {
        // If we don't have a buffer we don't have anything to render
        let Some(buffer) = &self.buffer else {
            return;
        };

        let mut row = rect.col;
        let mut col = rect.row;

        let min_col = rect.col;
        let max_col = rect.col.saturating_add(rect.area.cols);
        let min_row = rect.row;
        let max_row = rect.row.saturating_add(rect.area.rows);

        let start_line = buffer.cursor.line.saturating_sub(rect.area.rows / 2);

        let mut line_num: u16 = start_line;
        let mut char_num: u16 = 0;

        'lines: for line in buffer.rope.lines().skip(usize::from(start_line)) {
            for c in line.chars() {
                if c == '\n' {
                    row = row.saturating_add(1);
                    col = min_col;
                    line_num = line_num.saturating_add(1);
                    char_num = 0;
                } else if c == '\t' {
                    col = col.saturating_add(4);
                    char_num = char_num.saturating_add(4);
                } else {
                    if col >= max_col {
                        row = row.saturating_add(1);
                        col = min_col;
                        line_num = line_num.saturating_add(1);
                        char_num = 0;
                    }

                    if row >= max_row {
                        break 'lines;
                    }

                    let cursor = line_num == buffer.cursor.line && char_num == buffer.cursor.char;
                    frame.write(col, row, Cell::new(c, cursor, 0));
                    col = col.saturating_add(1);
                    char_num = char_num.saturating_add(1);
                }
            }
            row = row.saturating_add(1);
            col = min_col;
            line_num = line_num.saturating_add(1);
            char_num = 0;
        }
    }
}
