use std::{
    fs, io, ops,
    path::{Path, PathBuf},
};

use crop::Rope;

use crate::{grapheme::Grapheme, view::layout::Glyph};

#[derive(Debug, Clone, Copy)]
pub struct Cursor {
    pub from_byte: usize,
    pub to_byte: usize,
}

impl Cursor {
    #[must_use]
    pub fn new(from_byte: usize, to_byte: usize) -> Self {
        Self { from_byte, to_byte }
    }

    #[must_use]
    pub fn overlaps(&self, range: ops::Range<usize>) -> bool {
        self.from_byte < range.end && range.start < self.to_byte
    }

    pub fn set_glyph(&mut self, glyph: Glyph) {
        self.from_byte = glyph.from_byte;
        self.to_byte = glyph.to_byte;
    }
}

#[derive(Debug, Clone)]
pub struct Buffer {
    pub path: Option<PathBuf>,
    pub cursor: Cursor,
    pub rope: Rope,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            path: None,
            cursor: Cursor::new(0, 1),
            rope: Rope::new(),
        }
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let content = fs::read(&path)?;

        // Should never be in prod im just lazy and don't feel like making real errors
        let content = str::from_utf8(&content).expect("Failed to parse utf8");

        let mut rope_builder = crop::RopeBuilder::new();
        rope_builder.append(content);

        Ok(Self {
            path: Some(path.as_ref().to_path_buf()),
            cursor: Cursor::new(0, 1),
            rope: rope_builder.build(),
        })
    }

    pub fn get_grapheme(&self, range: ops::Range<usize>) -> Grapheme {
        Grapheme::from_bytes_iter(self.rope.byte_slice(range).bytes())
    }
}
