/// Describes a single edit, so positions held outside the buffer can follow it.
///
/// It only holds lengths, the text itself lives in the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change {
    at: usize,
    removed_len: usize,
    inserted_len: usize,
}

impl Change {
    #[must_use]
    pub(crate) fn new(at: usize, removed_len: usize, inserted_len: usize) -> Self {
        Self {
            at,
            removed_len,
            inserted_len,
        }
    }

    /// Where a byte position from before the edit is after it.
    ///
    /// A position at or inside the edited range moves past the inserted text, so a cursor
    /// stays on the grapheme it was on while typing in front of it.
    #[must_use]
    pub fn map_pos(&self, pos: usize) -> usize {
        let removed_end = self.at.saturating_add(self.removed_len);
        if pos < self.at {
            pos
        } else if pos < removed_end {
            self.at.saturating_add(self.inserted_len)
        } else {
            pos.saturating_sub(self.removed_len)
                .saturating_add(self.inserted_len)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_pos_insert() {
        // "ab|cd" insert "XY" at 2
        let change = Change::new(2, 0, 2);
        assert_eq!(change.map_pos(1), 1, "before");
        assert_eq!(change.map_pos(2), 4, "at, moves past the inserted text");
        assert_eq!(change.map_pos(3), 5, "after");
    }

    #[test]
    fn map_pos_replace() {
        // "ab[cd]ef" replace "cd" with "X"
        let change = Change::new(2, 2, 1);
        assert_eq!(change.map_pos(1), 1, "before");
        assert_eq!(change.map_pos(2), 3, "at");
        assert_eq!(change.map_pos(3), 3, "inside");
        assert_eq!(change.map_pos(4), 3, "right after");
        assert_eq!(change.map_pos(5), 4, "after");
    }

    #[test]
    fn map_pos_delete() {
        // "ab[cd]ef" delete "cd"
        let change = Change::new(2, 2, 0);
        assert_eq!(change.map_pos(3), 2, "inside snaps to the start");
        assert_eq!(change.map_pos(4), 2, "right after");
        assert_eq!(change.map_pos(5), 3, "after");
    }
}
