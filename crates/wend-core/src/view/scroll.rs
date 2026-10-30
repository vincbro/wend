use super::{
    Direction, View,
    layout::{Layout, ScrollOffset, SlotPosition},
};

impl View {
    /// Returns the index of the view's last row.
    pub(super) fn bottom_row(&self) -> usize {
        // The terminal can report 0 rows
        self.rect.area.height().saturating_sub(1)
    }

    /// Returns how many rows to keep between the head and the top or bottom of the view, at
    /// most half of it so the head can always sit outside both margins.
    fn margin(&self) -> usize {
        self.scroll_margin.min(self.bottom_row() / 2)
    }

    pub(super) fn ensure_cursor_visible(&mut self) {
        // The buffer or area might have changed since the last rebuild
        self.refresh_layout();

        let head = self.selection.head();
        if self.layout.find_byte(head).is_none() {
            self.jump_to(head);
        }
        // Only an empty area has nowhere to show the head
        let Some(SlotPosition { row, .. }) = self.layout.find_byte(head) else {
            return;
        };

        // Scrolls just far enough that the head stops right at the margin
        let height = self.rect.area.height();
        let margin = self.margin();
        if row + margin >= height {
            self.scroll(Direction::Forward, row + margin + 1 - height);
        } else if row < margin {
            self.scroll(Direction::Backward, margin - row);
        }
    }

    /// Puts the line holding `byte` on top, then scrolls a screen at a time through it until
    /// `byte` shows up. A jump never passes it, the margin pulls it into place below.
    fn jump_to(&mut self, byte: usize) {
        let height = self.rect.area.height();
        self.rebuild_at(ScrollOffset::new(self.buffer.rope().line_of_byte(byte), 0));
        while self.layout.find_byte(byte).is_none() {
            let prev_offset = self.layout.scroll_offset();
            self.rebuild_at(ScrollOffset::new(
                prev_offset.line,
                prev_offset.row + height.max(1),
            ));
            if self.layout.scroll_offset() == prev_offset {
                break;
            }
        }
    }

    /// Scrolls up to `rows` rows with a single rebuild and returns how many it scrolled.
    ///
    /// Backward stops at the top of the buffer. Forward stops once the last row on
    /// screen is on top, the rows past it aren't laid out.
    pub(super) fn scroll(&mut self, direction: Direction, rows: usize) -> usize {
        let (scroll_offset, scrolled) = match direction {
            Direction::Forward => self.offset_below(rows),
            Direction::Backward => self.offset_above(rows),
        };
        if scrolled > 0 {
            self.rebuild_at(scroll_offset);
        }
        scrolled
    }

    /// Returns the offset `rows` rows down, read from the layout, and how many rows that is.
    fn offset_below(&self, rows: usize) -> (ScrollOffset, usize) {
        let rows = rows.min(self.layout.last_row().unwrap_or(0));
        let scroll_offset = self
            .layout
            .offset_of_row(rows)
            .unwrap_or(self.layout.scroll_offset());
        (scroll_offset, rows)
    }

    /// Returns the scroll offset `rows` rows up, and how many rows that is.
    fn offset_above(&self, rows: usize) -> (ScrollOffset, usize) {
        let ScrollOffset { mut line, mut row } = self.layout.scroll_offset();
        let mut left = rows;
        // A line at a time, onto the last row of the line above
        while left > row {
            let Some(above) = line.checked_sub(1) else {
                return (ScrollOffset::default(), rows - left + row);
            };
            left -= row + 1;
            line = above;
            row = Layout::last_row_of_line(&self.buffer, self.rect.area.cols, line);
        }
        (ScrollOffset::new(line, row - left), rows)
    }

    /// Rebuilds the layout if anything it depends on changed since the last rebuild.
    pub(super) fn refresh_layout(&mut self) {
        if self.layout_dirty {
            self.rebuild_at(self.layout.scroll_offset());
        }
    }

    pub(super) fn rebuild_at(&mut self, scroll_offset: ScrollOffset) {
        // Edits and reverts can make the buffer shorter than where we scrolled to, the
        // layout clamps the row the same way
        let line = scroll_offset.line.min(self.buffer.rope().line_len());
        self.layout.rebuild(
            &self.buffer,
            self.rect.area,
            ScrollOffset::new(line, scroll_offset.row),
        );
        self.layout_dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::{Motion, tests};

    #[test]
    fn scrolls_through_a_wrapped_line_a_row_at_a_time() {
        // Width 4, one line: "aaaa" then rows of "↩ aa". Height 5 gives a margin of 2.
        let mut view = tests::view(&"a".repeat(40), 4, 5);
        let mut rows = vec![];
        for _ in 0..12 {
            view.move_selection(Motion::Vertical(Direction::Forward), false);
            let pos = view.layout.find_byte(view.selection.head()).unwrap();
            rows.push((pos.row, view.layout.scroll_offset()));
        }
        // Down to the margin, then the head stays on it and the view follows row by row
        let expected: Vec<_> = (1..=12)
            .map(|step: usize| {
                let row = step.min(2);
                (row, ScrollOffset::new(0, step - row))
            })
            .collect();
        assert_eq!(rows, expected);

        // And back up, the head stays on the top margin until the view reaches the top
        view.move_selection(Motion::Vertical(Direction::Backward), false);
        view.move_selection(Motion::Vertical(Direction::Backward), false);
        view.move_selection(Motion::Vertical(Direction::Backward), false);
        let pos = view.layout.find_byte(view.selection.head()).unwrap();
        assert_eq!(
            (pos.row, view.layout.scroll_offset()),
            (2, ScrollOffset::new(0, 7)),
            "each step up scrolls one row, the head stays on the top margin"
        );
    }

    #[test]
    fn scrolls_up_onto_the_last_row_of_the_line_above() {
        // Width 4: line 0 is "aaaa", "↩ a\n", line 1 is "b"
        let mut view = tests::view("aaaaa\nb\nc\nd", 4, 3);
        view.rebuild_at(ScrollOffset::new(1, 0));
        assert_eq!(view.scroll(Direction::Backward, 1), 1, "line 0 is above");
        assert_eq!(
            view.layout.scroll_offset(),
            ScrollOffset::new(0, 1),
            "onto the wrapped row of line 0, not its first row"
        );
        assert_eq!(
            view.scroll(Direction::Backward, 1),
            1,
            "line 0 has a row left"
        );
        assert_eq!(
            view.layout.scroll_offset(),
            ScrollOffset::new(0, 0),
            "then up through line 0's rows"
        );
        assert_eq!(view.scroll(Direction::Backward, 1), 0, "already at the top");
    }

    #[test]
    fn scrolls_many_rows_at_once() {
        // Width 4: line 0 is "aaaa", "↩ a\n", then one row per line
        let mut view = tests::view("aaaaa\nb\nc\nd\ne", 4, 3);
        assert_eq!(
            view.scroll(Direction::Forward, 3),
            2,
            "stops at the last row"
        );
        assert_eq!(
            view.layout.scroll_offset(),
            ScrollOffset::new(1, 0),
            "past both rows of line 0"
        );
        assert_eq!(
            view.scroll(Direction::Backward, 5),
            2,
            "stops at the top of the buffer"
        );
        assert_eq!(view.layout.scroll_offset(), ScrollOffset::new(0, 0));
    }

    #[test]
    fn jumps_into_the_middle_of_a_tall_line() {
        // One line far taller than the screen, the end of it can still be reached
        let mut view = tests::view(&"a".repeat(200), 4, 5);
        view.move_selection(Motion::File(Direction::Forward), false);
        assert_eq!(view.selection.head(), 199, "the head is not pulled back");
        let pos = view.layout.find_byte(view.selection.head()).unwrap();
        assert_eq!(pos.row, 2, "on the bottom margin");
    }
}
