use std::{path::Path, sync::Arc};

use crate::{buffer::Buffer, syntax::Grammar};

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

    /// Gives the buffer `grammar`, see `Buffer::attach`.
    pub(super) fn attach(&mut self, grammar: &Arc<Grammar>) {
        self.buffer.attach(grammar);
    }

    /// Hands out the buffer to change, for tests that set one up by hand. The editor changes a
    /// document's buffer through the methods above, so its checks run.
    #[cfg(test)]
    pub(super) fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    /// Gives up the path and the buffer.
    pub(super) fn into_parts(self) -> (Option<Box<Path>>, Buffer) {
        (self.path, self.buffer)
    }
}
