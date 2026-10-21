use std::{
    fs, io,
    path::{Path, PathBuf},
    str::from_utf8,
};

pub struct Cursor {
    pub char: u16,
    pub line: u16,
}

impl Cursor {
    pub fn new(char: u16, line: u16) -> Self {
        Self { char, line }
    }
}

pub struct Buffer {
    pub file: PathBuf,
    pub cursor: Cursor,
    pub rope: crop::Rope,
}

impl Buffer {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let content = fs::read(&path)?;

        // Should never be in prod im just lazy and don't feel like making real errors
        let content = str::from_utf8(&content).expect("Failed to parse utf8");

        let mut rope_builder = crop::RopeBuilder::new();
        rope_builder.append(content);

        Ok(Self {
            file: path.as_ref().to_path_buf(),
            cursor: Cursor::new(0, 0),
            rope: rope_builder.build(),
        })
    }

    pub fn up(&mut self) {
        self.cursor.line = self.cursor.line.saturating_sub(1)
    }

    pub fn down(&mut self) {
        self.cursor.line = self.cursor.line.saturating_add(1);
    }

    pub fn left(&mut self) {
        self.cursor.char = self.cursor.char.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor.char = self.cursor.char.saturating_add(1);
    }
}
