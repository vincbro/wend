use std::{mem, ops};

use super::{Highlighter, Layout, Selection, Slot, SlotPosition, View, gutter};
use crate::frame::{Cell, Frame, Grapheme};

impl View {
    /// Writes the view's cells into `frame`, leaving the rest of it alone.
    ///
    /// # Panics
    ///
    /// If the view's rect reaches outside `frame`.
    pub fn render(&mut self, frame: &mut Frame) {
        self.refresh_layout();
        let text_rect = self.text_rect();
        let rows_with_text = self.layout.last_row().map_or(0, |row| row + 1);
        let head = self.head_position().and_then(|slot| {
            let metadata = self.layout.row_metadata(slot.row)?;
            Some(gutter::Head {
                row: slot.row,
                line: metadata.line,
            })
        });
        self.gutter.render(frame, self.rect, rows_with_text, head);

        let selected = self.selection_range();
        let mut open = mem::take(&mut self.open_highlights);
        // The highlights borrow the buffer for as long as the loop runs, so the loop only
        // reads the other fields, and doesn't call methods that take the whole view
        let spans = self.buffer.highlights(self.visible_bytes());
        let mut highlighter = Highlighter::new(spans, &mut open);
        for (pos, slot) in self.layout.slots() {
            let mut cell = Self::cell(&self.layout, self.selection, slot, pos, &selected);
            // Tails are blanks, only the glyph itself has a color
            if let Slot::Glyph(glyph) = slot {
                cell.highlight = highlighter.at(glyph.from_byte);
            }
            frame.write(Self::cell_position(text_rect, pos), cell);
        }

        self.open_highlights = open;
    }

    /// The bytes of the text the layout shows, from the first row's start to the last row's end.
    fn visible_bytes(&self) -> ops::Range<usize> {
        let mut rows = self.layout.rows();
        let Some(first) = rows.next() else {
            return 0..0;
        };
        let last = rows.last().unwrap_or(first);
        first.metadata.from_byte..last.metadata.to_byte
    }

    /// The cell the slot at `pos` renders as. `selected` is the selection's byte range.
    ///
    /// Takes the parts of the view it reads instead of the view, so `render` can keep the
    /// buffer borrowed meanwhile.
    fn cell(
        layout: &Layout,
        selection: Selection,
        slot: Slot,
        pos: SlotPosition,
        selected: &ops::Range<usize>,
    ) -> Cell {
        let blank = Cell {
            grapheme: Grapheme::blank(),
            ..Cell::default()
        };
        let Some(glyph) = layout.glyph(slot) else {
            return match slot {
                Slot::Marker(c) => Cell {
                    grapheme: Grapheme::from_char(c),
                    ..blank
                },
                _ => blank,
            };
        };

        // A tail is covered when the head prints wide enough to reach it, the rest (a tab,
        // a too long grapheme shown as U+FFFD) print as blanks
        let (grapheme, covered) = match slot {
            Slot::Tail(head_index) => {
                let offset = layout.slot_index(pos) - head_index;
                (Grapheme::blank(), offset < glyph.grapheme.width())
            }
            _ => (glyph.grapheme, false),
        };

        Cell {
            grapheme,
            cursor: glyph.to_range().contains(&selection.head()),
            selected: selected.contains(&glyph.from_byte),
            covered,
            // Set by render, which walks the cells in order
            highlight: None,
        }
    }
}
