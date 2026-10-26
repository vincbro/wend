use std::{
    fs, io, ops,
    path::{Path, PathBuf},
};

use crop::Rope;

use crate::grapheme::Grapheme;

#[derive(Debug, Clone)]
pub struct Buffer {
    pub path: Option<PathBuf>,
    pub rope: Rope,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            path: None,
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
            rope: rope_builder.build(),
        })
    }

    pub fn write<T: AsRef<str>>(&mut self, byte_idx: usize, value: T) {
        self.rope.insert(byte_idx, value);
    }

    pub fn delete(&mut self, from_byte: usize, to_byte: usize) {
        self.rope.delete(from_byte..to_byte);
    }

    pub fn get_grapheme(&self, range: ops::Range<usize>) -> Grapheme {
        Grapheme::from_bytes_iter(self.rope.byte_slice(range).bytes())
    }
}
