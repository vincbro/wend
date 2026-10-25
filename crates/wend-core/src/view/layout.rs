use crate::{area::Area, buffer::Buffer};

use std::ops;

use unicode_width::UnicodeWidthStr;

const TAB_WIDTH: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Glyph {
    pub from_byte: usize,
    pub to_byte: usize,
    pub tails: usize,
}

impl Glyph {
    #[must_use]
    pub(super) fn new(from_byte: usize, to_byte: usize, tails: usize) -> Self {
        Self {
            from_byte,
            to_byte,
            tails,
        }
    }

    #[must_use]
    pub(super) fn as_range(&self) -> ops::Range<usize> {
        self.from_byte..self.to_byte
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Slot {
    Glyph(Glyph),
    Tail(usize),
    Misc(char),
    Blank,
}

impl Slot {
    #[must_use]
    pub const fn is_glyph(self) -> bool {
        matches!(self, Self::Glyph(_))
    }

    #[must_use]
    pub const fn is_tail(self) -> bool {
        matches!(self, Self::Tail { .. })
    }

    #[must_use]
    pub const fn is_misc(self) -> bool {
        matches!(self, Self::Misc(_))
    }

    #[must_use]
    pub const fn is_blank(self) -> bool {
        matches!(self, Self::Blank)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Row<'a> {
    pub meta: RowMeta,
    pub slots: &'a [Slot],
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RowMeta {
    pub line: usize,
    pub from_byte: usize,
    pub to_byte: usize,
    // How many slots unitl the end of the line
    pub len: usize,
}

#[derive(Debug, Clone)]
pub(super) struct Layout {
    slots: Vec<Slot>,
    row_meta: Vec<Option<RowMeta>>,
    area: Area,
    // From which line we start from
    scroll_offset: usize,

    row_ptr: usize,
    col_ptr: usize,
    line_ptr: usize,
    row_byte_ptr: usize,
    byte_ptr: usize,
}

impl Layout {
    #[must_use]
    pub fn new(area: Area, scroll_offset: usize) -> Self {
        let mut layout = Self {
            slots: Vec::with_capacity(area.len()),
            row_meta: Vec::with_capacity(area.height()),
            area,
            scroll_offset,

            row_ptr: 0,
            col_ptr: 0,
            line_ptr: 0,

            row_byte_ptr: 0,
            byte_ptr: 0,
        };
        layout.resize(area, scroll_offset);
        layout
    }

    // Gets the glyph, none if it's Blank
    #[must_use]
    pub(super) fn glyph(&self, slot: Slot) -> Option<Glyph> {
        match slot {
            Slot::Glyph(glyph) => Some(glyph),
            Slot::Tail(slot_idx) => self.glyph(self.slot(slot_idx)?),
            Slot::Misc(_) | Slot::Blank => None,
        }
    }

    #[must_use]
    pub(super) fn slot(&self, slot_idx: usize) -> Option<Slot> {
        self.slots.get(slot_idx).copied()
    }

    #[must_use]
    pub(super) fn slot_index(&self, col: usize, row: usize) -> usize {
        col + row * self.area.width()
    }

    #[must_use]
    pub(super) fn row(&self, index: usize) -> Option<Row<'_>> {
        let start = index * self.area.width();
        let end = start + self.area.width();
        Some(Row {
            meta: self.row_meta.get(index).copied()??,
            slots: self.slots.get(start..end)?,
        })
    }

    pub(super) fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        (0..self.area.height()).filter_map(|idx| self.row(idx))
    }

    fn new_line(&mut self) {
        self.seal();
        self.line_ptr = self.line_ptr.saturating_add(1);
    }

    /// Seals the current row, appends row meta data.
    fn seal(&mut self) {
        self.row_meta[self.row_ptr] = Some(RowMeta {
            line: self.line_ptr,
            from_byte: self.row_byte_ptr,
            to_byte: self.byte_ptr,
            len: self.col_ptr,
        });

        for _ in self.col_ptr..self.area.width() {
            let idx = self.col_ptr + self.row_ptr * self.area.width();
            let Some(slot_mut) = self.slots.get_mut(idx) else {
                panic!("Should not be able to overflow");
            };
            *slot_mut = Slot::Blank;
            self.col_ptr = self.col_ptr.saturating_add(1);
        }

        self.row_byte_ptr = self.byte_ptr;

        self.col_ptr = 0;
        self.row_ptr = self.row_ptr.saturating_add(1);
    }

    fn write_grapheme(&mut self, grapheme: &str) {
        let mut glyph = Glyph::new(
            self.byte_ptr,
            self.byte_ptr.saturating_add(grapheme.len()),
            grapheme.width().saturating_sub(1),
        );
        if grapheme == "\t" {
            let jumps = if self.col_ptr.is_multiple_of(TAB_WIDTH) {
                TAB_WIDTH
            } else {
                TAB_WIDTH - (self.col_ptr % TAB_WIDTH)
            }
            .saturating_sub(1);
            glyph.tails = jumps;
            self.write(Slot::Glyph(glyph));
        } else {
            self.write(Slot::Glyph(glyph));
            if grapheme.ends_with('\n') {
                self.new_line();
            }
        }
    }

    /// Writes a single slot, handles auto sealing and overflow protection.
    fn write(&mut self, slot: Slot) {
        if self.row_ptr >= self.area.height() || self.col_ptr >= self.area.width() {
            return;
        }

        let idx = self.slot_index(self.col_ptr, self.row_ptr);
        let Some(slot_mut) = self.slots.get_mut(idx) else {
            panic!("Should not be able to overflow");
        };
        *slot_mut = slot;

        self.col_ptr = self.col_ptr.saturating_add(1);

        if let Slot::Glyph(glyph) = slot {
            self.byte_ptr = glyph.to_byte;
            for _ in 0..glyph.tails {
                self.write(Slot::Tail(idx));
            }
        }

        if self.col_ptr >= self.area.width() {
            self.seal();
            self.write(Slot::Misc('↩'));
            self.write(Slot::Blank);
        }
    }

    fn resize(&mut self, area: Area, scroll_offset: usize) {
        self.area = area;
        self.scroll_offset = scroll_offset;

        self.row_meta.clear();
        self.row_meta.resize_with(self.area.height(), || None);
        self.slots.clear();
        self.slots.resize_with(area.len(), || Slot::Blank);

        self.col_ptr = 0;
        self.row_ptr = 0;
        self.line_ptr = scroll_offset;
    }

    pub(super) fn rebuild(&mut self, buffer: &Buffer, area: Area, scroll_offset: usize) {
        self.resize(area, scroll_offset);

        let start = buffer.rope.byte_of_line(self.scroll_offset);
        self.byte_ptr = start;
        self.row_byte_ptr = start;

        for grapheme in buffer.rope.byte_slice(start..).graphemes() {
            if self.row_ptr >= self.area.height() {
                break;
            }

            self.write_grapheme(&grapheme);

            // Here we will add linking logic for multiline slots
        }

        if self.col_ptr > 0 {
            self.seal();
        }
    }
}
