use std::path::Path;

use crate::buffer::Buffer;

/// A buffer and the file it came from, which a buffer doesn't know. A document without a path
/// was never loaded from one.
#[derive(Debug)]
pub(super) struct Document {
    path: Option<Box<Path>>,
    buffer: Buffer,
}

impl Document {
    pub(super) fn new(path: Option<Box<Path>>, buffer: Buffer) -> Self {
        Self { path, buffer }
    }

    pub(super) fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub(super) fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub(super) fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    /// Gives up the path and the buffer.
    pub(super) fn into_parts(self) -> (Option<Box<Path>>, Buffer) {
        (self.path, self.buffer)
    }
}
