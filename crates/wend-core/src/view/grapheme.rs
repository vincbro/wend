use std::ops;

use super::{Glyph, View};

impl View {
    /// Returns the glyph holding `byte`, none if it is off screen or the layout is out of date.
    fn glyph_on_screen(&self, byte: usize) -> Option<Glyph> {
        if self.layout_dirty {
            return None;
        }
        self.layout.glyph_at(self.layout.find_byte(byte)?)
    }

    /// Returns where the grapheme holding `byte` starts, the buffer's end for anything past it.
    pub(super) fn grapheme_start(&self, byte: usize) -> usize {
        self.glyph_on_screen(byte)
            .map_or_else(|| self.rope_grapheme_start(byte), |glyph| glyph.from_byte)
    }

    /// Walks the line from its start, so this costs as much as the line is long.
    pub(super) fn rope_grapheme_start(&self, byte: usize) -> usize {
        let rope = self.buffer.rope();
        let mut byte = byte.min(rope.byte_len());
        // Line lookups need a char boundary, back up to one, at most 3 bytes in UTF-8
        while !rope.is_char_boundary(byte) {
            byte -= 1;
        }
        let mut start = rope.byte_of_line(rope.line_of_byte(byte));
        for grapheme in rope.byte_slice(start..).graphemes() {
            let end = start + grapheme.len();
            if end > byte {
                return start;
            }
            start = end;
        }
        start
    }

    /// Returns where the grapheme starting at `start` ends. The end of buffer slot is one byte
    /// wide, see `Layout::rebuild`.
    pub(super) fn grapheme_end(&self, start: usize) -> usize {
        self.glyph_on_screen(start)
            .map_or_else(|| self.rope_grapheme_end(start), |glyph| glyph.to_byte)
    }

    /// Returns where the grapheme starting at `start` ends, inside the text even for the end
    /// of buffer slot.
    pub(super) fn clamped_grapheme_end(&self, start: usize) -> usize {
        self.grapheme_end(start).min(self.buffer.rope().byte_len())
    }

    pub(super) fn rope_grapheme_end(&self, start: usize) -> usize {
        let len = self
            .buffer
            .rope()
            .byte_slice(start..)
            .graphemes()
            .next()
            .map_or(1, |grapheme| grapheme.len());
        start + len
    }

    /// Returns the bytes the selection covers, from its first grapheme through its last.
    pub(super) fn selection_range(&self) -> ops::Range<usize> {
        self.selection.start()..self.clamped_grapheme_end(self.selection.end())
    }

    /// Returns where the text of `line` ends, before its line break.
    pub(super) fn line_end(&self, line: usize) -> usize {
        let rope = self.buffer.rope();
        if line < rope.line_len() {
            rope.byte_of_line(line) + rope.line(line).byte_len()
        } else {
            rope.byte_len()
        }
    }
}
