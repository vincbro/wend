use std::{mem, ops};

use super::{Highlighter, Selection, Slot, SlotPosition, View, Wrap, gutter};
use crate::{
    frame::{Cell, Content, Grapheme, Rect, Style},
    layout::{Canvas, Element, Layout},
};

impl Element for View {
    /// Takes all the room it is given, and centers the text in it itself.
    fn layout(&self) -> Layout {
        Layout::FILL
    }

    /// Writes the view's cells into `canvas`, and puts the cursor where the head is.
    fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
        self.resize(rect);
        // Scrolls to the cursor first, so what is drawn is where the cursor is
        let cursor = self.cursor_position();
        self.refresh_wrap();
        let text_rect = self.text_rect();
        let rows_with_text = self.wrap.last_row().map_or(0, |row| row + 1);
        let head = self.head_position().and_then(|slot| {
            let metadata = self.wrap.row_metadata(slot.row)?;
            Some(gutter::Head {
                row: slot.row,
                line: metadata.line,
            })
        });
        self.gutter
            .render(canvas, self.block(), rows_with_text, head);

        let selected = self.selection_range();
        let mut open = mem::take(&mut self.open_highlights);
        // The highlights borrow the buffer for as long as the loop runs, so the loop only
        // reads the other fields, and doesn't call methods that take the whole view
        let spans = self.buffer.highlights(self.visible_bytes());
        let mut highlighter = Highlighter::new(spans, &mut open);
        for (pos, slot) in self.wrap.slots() {
            let mut cell = Self::cell(&self.wrap, self.selection, slot, pos, &selected);
            // Tails are blanks, only the glyph itself has a color
            if let Slot::Glyph(glyph) = slot
                && let Some(highlight) = highlighter.at(glyph.start())
            {
                cell.style.content = Content::Text(highlight);
            }
            canvas.write(Self::cell_position(text_rect, pos), cell);
        }
        if let Some(cursor) = cursor {
            canvas.set_cursor(cursor);
        }

        self.open_highlights = open;
    }
}

impl View {
    /// The bytes of the text the wrap shows, from the first row's start to the last row's end.
    fn visible_bytes(&self) -> ops::Range<usize> {
        let mut rows = self.wrap.rows();
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
        wrap: &Wrap,
        selection: Selection,
        slot: Slot,
        pos: SlotPosition,
        selected: &ops::Range<usize>,
    ) -> Cell {
        let blank = Cell {
            grapheme: Grapheme::blank(),
            ..Cell::default()
        };
        let Some(glyph) = wrap.glyph(slot) else {
            return match slot {
                Slot::Marker(c) => Cell {
                    grapheme: Grapheme::from_char(c),
                    style: Style {
                        content: Content::Marker,
                        ..Style::default()
                    },
                    ..blank
                },
                _ => blank,
            };
        };

        // A tail is covered when the head prints wide enough to reach it, the rest (a tab,
        // a too long grapheme shown as U+FFFD) print as blanks
        let (grapheme, covered) = match slot {
            Slot::Tail(head_index) => {
                let offset = wrap.slot_index(pos) - head_index;
                (Grapheme::blank(), offset < glyph.grapheme().width())
            }
            _ => (glyph.grapheme(), false),
        };

        Cell {
            grapheme,
            // The content is set by render, which walks the cells in order
            style: Style {
                content: Content::Plain,
                selected: selected.contains(&glyph.start()),
                cursor: glyph.to_range().contains(&selection.head()),
            },
            covered,
            ..Cell::default()
        }
    }
}
