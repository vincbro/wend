use std::num::NonZeroUsize;

use crop::Rope;

/// The most bytes of text a read-only box's text is compared against its owner's in
/// `check_invariants`.
const READ_ONLY_CHECK_LIMIT: usize = 1 << 20;

/// What restricts the text of a box: how many lines it may have, and whether the user may
/// change it at all.
///
/// A read-only box holds the text its owner last gave it, and `check_invariants` checks the
/// text against it. The owner says what the text is now with `owner_changed_text`.
#[derive(Debug, Clone, Default)]
pub(super) struct Limits {
    /// The most lines the text has, none for a text box that holds a whole buffer.
    line_limit: Option<NonZeroUsize>,
    /// The text the owner last gave a read-only box, none for one that can be changed. Keys and
    /// actions are refused when they would change the text, so only the owner changes it.
    read_only: Option<Rope>,
}

/// Returns how many lines `rope` shows, which is one more than its line breaks.
pub(super) fn visible_lines(rope: &Rope) -> usize {
    let ends_in_break = rope
        .byte_len()
        .checked_sub(1)
        .is_some_and(|last| rope.byte(last) == b'\n');
    rope.line_len().max(1) + usize::from(ends_in_break)
}

impl Limits {
    pub(super) fn line_limit(&self) -> Option<NonZeroUsize> {
        self.line_limit
    }

    pub(super) fn set_line_limit(&mut self, limit: NonZeroUsize) {
        self.line_limit = Some(limit);
    }

    pub(super) fn is_read_only(&self) -> bool {
        self.read_only.is_some()
    }

    /// Makes the text `text` of a read-only box, or lets the user change it again.
    pub(super) fn set_read_only(&mut self, read_only: bool, text: &Rope) {
        self.read_only = read_only.then(|| text.clone());
    }

    /// Takes in what the owner put in as the text of a read-only box.
    pub(super) fn owner_changed_text(&mut self, text: &Rope) {
        if self.read_only.is_some() {
            self.read_only = Some(text.clone());
        }
    }

    /// Checks `text` against both limits. Debug builds only.
    pub(super) fn check_invariants(&self, text: &Rope) {
        if !cfg!(debug_assertions) {
            return;
        }

        // Past a size the comparison costs as much as the text is long
        if let Some(owned) = &self.read_only
            && text.byte_len() <= READ_ONLY_CHECK_LIMIT
        {
            assert!(
                owned == text,
                "a read-only box refuses every edit, so only its owner changes its text"
            );
        }
        assert!(
            self.line_limit
                .is_none_or(|limit| visible_lines(text) <= limit.get()),
            "edits, new buffers and reverts are cut to the line limit"
        );
    }
}
