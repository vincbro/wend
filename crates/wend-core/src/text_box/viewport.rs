use std::num::NonZeroUsize;

use super::{
    Direction, TextBox,
    wrap::{ScrollOffset, SlotPosition, Wrap},
};
use crate::{buffer::Buffer, frame::Area};

/// Which part of a buffer shows in an area: the wrapped rows from the scroll offset on.
///
/// The wrap is only as new as the last rebuild, so what changes the text or the area marks it
/// dirty, and `refresh` rebuilds it. After every public call the wrap and the probe keep their
/// own invariants. `check_invariants` checks them.
#[derive(Debug, Clone)]
pub(super) struct Viewport {
    wrap: Wrap,
    dirty: bool,
    /// Scratch space for `Wrap::last_row_of_line`, kept so that its memory is reused.
    line_probe: Wrap,
    /// How many rows to keep between the head and the top or bottom of the area.
    margin: usize,
}

impl Viewport {
    pub(super) fn new(area: Area, tab_width: NonZeroUsize, margin: usize) -> Self {
        Self {
            wrap: Wrap::new(area, ScrollOffset::default(), tab_width),
            dirty: true,
            line_probe: Wrap::new(Area::new(0, 0), ScrollOffset::default(), tab_width),
            margin,
        }
    }

    pub(super) fn wrap(&self) -> &Wrap {
        &self.wrap
    }

    /// Whether the wrap is older than the text or the area it shows.
    pub(super) fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Says the text or the area changed, so the wrap is rebuilt before it is read.
    pub(super) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Follows the margin and the tab width of a new config.
    pub(super) fn set_config(&mut self, margin: usize, tab_width: NonZeroUsize) {
        self.margin = margin;
        self.wrap.set_tab_width(tab_width);
        self.line_probe.set_tab_width(tab_width);
        self.dirty = true;
    }

    /// Rebuilds the wrap if anything it depends on changed since the last rebuild.
    pub(super) fn refresh(&mut self, buffer: &Buffer, area: Area) {
        if self.dirty {
            self.rebuild_at(buffer, area, self.wrap.scroll_offset());
        }
    }

    pub(super) fn rebuild_at(&mut self, buffer: &Buffer, area: Area, scroll_offset: ScrollOffset) {
        // Edits and reverts can make the buffer shorter than where we scrolled to, the
        // wrap clamps the row the same way
        let line = scroll_offset.line.min(buffer.rope().line_len());
        self.wrap
            .rebuild(buffer, area, ScrollOffset::new(line, scroll_offset.row));
        self.dirty = false;
    }

    /// Returns how many rows to keep between the head and the top or bottom of the area, at
    /// most half of it so the head can always sit outside both margins.
    fn margin(&self, area: Area) -> usize {
        self.margin.min(bottom_row(area) / 2)
    }

    /// Scrolls until `head` is on screen and outside the margins, if the area has room for it.
    pub(super) fn ensure_visible(&mut self, buffer: &Buffer, area: Area, head: usize) {
        // The buffer or area might have changed since the last rebuild
        self.refresh(buffer, area);

        if self.wrap.find_byte(head).is_none() {
            self.jump_to(buffer, area, head);
        }
        // Only an empty area has nowhere to show the head
        let Some(SlotPosition { row, .. }) = self.wrap.find_byte(head) else {
            return;
        };

        let height = area.height();
        let margin = self.margin(area);
        if row + margin >= height {
            self.scroll(buffer, area, Direction::Forward, row + margin + 1 - height);
        } else if row < margin {
            self.scroll(buffer, area, Direction::Backward, margin - row);
        }
    }

    /// Puts the line holding `byte` on top, then scrolls a screen at a time through it until
    /// `byte` shows up. A jump never passes it, the margin pulls it into place below.
    fn jump_to(&mut self, buffer: &Buffer, area: Area, byte: usize) {
        let height = area.height();
        self.rebuild_at(
            buffer,
            area,
            ScrollOffset::new(buffer.rope().line_of_byte(byte), 0),
        );
        while self.wrap.find_byte(byte).is_none() {
            let prev_offset = self.wrap.scroll_offset();
            self.rebuild_at(
                buffer,
                area,
                ScrollOffset::new(prev_offset.line, prev_offset.row + height.max(1)),
            );
            if self.wrap.scroll_offset() == prev_offset {
                break;
            }
        }
    }

    /// Scrolls up to `rows` rows with a single rebuild and returns how many it scrolled.
    ///
    /// Backward stops at the top of the buffer. Forward stops once the last row on
    /// screen is on top, the rows past it aren't laid out.
    pub(super) fn scroll(
        &mut self,
        buffer: &Buffer,
        area: Area,
        direction: Direction,
        rows: usize,
    ) -> usize {
        let (scroll_offset, scrolled) = match direction {
            Direction::Forward => self.offset_below(rows),
            Direction::Backward => self.offset_above(buffer, area, rows),
        };
        if scrolled > 0 {
            self.rebuild_at(buffer, area, scroll_offset);
        }
        scrolled
    }

    /// Returns the offset `rows` rows down, read from the wrap, and how many rows that is.
    fn offset_below(&self, rows: usize) -> (ScrollOffset, usize) {
        let rows = rows.min(self.wrap.last_row().unwrap_or(0));
        let scroll_offset = self
            .wrap
            .offset_of_row(rows)
            .unwrap_or(self.wrap.scroll_offset());
        (scroll_offset, rows)
    }

    /// Returns the scroll offset `rows` rows up, and how many rows that is.
    fn offset_above(&mut self, buffer: &Buffer, area: Area, rows: usize) -> (ScrollOffset, usize) {
        let ScrollOffset { mut line, mut row } = self.wrap.scroll_offset();
        let mut left = rows;
        // A line at a time, onto the last row of the line above
        while left > row {
            let Some(above) = line.checked_sub(1) else {
                return (ScrollOffset::default(), rows - left + row);
            };
            left -= row + 1;
            line = above;
            row = self.line_probe.last_row_of_line(buffer, area.cols, line);
        }
        (ScrollOffset::new(line, row - left), rows)
    }

    pub(super) fn check_invariants(&self) {
        if !cfg!(debug_assertions) {
            return;
        }

        self.wrap.check_invariants();
        self.line_probe.check_invariants();
    }
}

/// The last row of an area, the terminal can report 0 rows.
fn bottom_row(area: Area) -> usize {
    area.height().saturating_sub(1)
}

impl TextBox {
    pub(super) fn bottom_row(&self) -> usize {
        bottom_row(self.rect.area)
    }

    pub(super) fn ensure_cursor_visible(&mut self) {
        self.viewport
            .ensure_visible(&self.buffer, self.rect.area, self.selection.head());
        self.clamp_desired_col();
    }

    pub(super) fn scroll(&mut self, direction: Direction, rows: usize) -> usize {
        let scrolled = self
            .viewport
            .scroll(&self.buffer, self.rect.area, direction, rows);
        self.clamp_desired_col();
        scrolled
    }

    /// Rebuilds the wrap if anything it depends on changed since the last rebuild.
    pub(super) fn refresh_wrap(&mut self) {
        self.viewport.refresh(&self.buffer, self.rect.area);
        self.clamp_desired_col();
    }

    pub(super) fn rebuild_at(&mut self, scroll_offset: ScrollOffset) {
        self.viewport
            .rebuild_at(&self.buffer, self.rect.area, scroll_offset);
        self.clamp_desired_col();
    }

    /// Keeps the column up and down aim for inside the width of the area.
    fn clamp_desired_col(&mut self) {
        self.desired_col = self
            .desired_col
            .min(self.rect.area.width().saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_box::{Motion, tests};

    #[test]
    fn scrolls_through_a_wrapped_line_a_row_at_a_time() {
        // Width 4, one line: "aaaa" then rows of "↩ aa". Height 5 gives a margin of 2.
        let mut view = tests::text_box(&"a".repeat(40), 4, 5);
        let mut rows = vec![];
        for _ in 0..12 {
            view.move_selection(Motion::Vertical(Direction::Forward), false);
            let pos = view
                .viewport
                .wrap()
                .find_byte(view.selection.head())
                .unwrap();
            rows.push((pos.row, view.viewport.wrap().scroll_offset()));
        }
        // Down to the margin, then the head stays on it and the text follows row by row
        let expected: Vec<_> = (1..=12)
            .map(|step: usize| {
                let row = step.min(2);
                (row, ScrollOffset::new(0, step - row))
            })
            .collect();
        assert_eq!(rows, expected);

        // And back up, the head stays on the top margin until the text reaches the top
        view.move_selection(Motion::Vertical(Direction::Backward), false);
        view.move_selection(Motion::Vertical(Direction::Backward), false);
        view.move_selection(Motion::Vertical(Direction::Backward), false);
        let pos = view
            .viewport
            .wrap()
            .find_byte(view.selection.head())
            .unwrap();
        assert_eq!(
            (pos.row, view.viewport.wrap().scroll_offset()),
            (2, ScrollOffset::new(0, 7)),
            "each step up scrolls one row, the head stays on the top margin"
        );
    }

    #[test]
    fn scrolls_up_onto_the_last_row_of_the_line_above() {
        // Width 4: line 0 is "aaaa", "↩ a\n", line 1 is "b"
        let mut view = tests::text_box("aaaaa\nb\nc\nd", 4, 3);
        view.rebuild_at(ScrollOffset::new(1, 0));
        assert_eq!(view.scroll(Direction::Backward, 1), 1, "line 0 is above");
        assert_eq!(
            view.viewport.wrap().scroll_offset(),
            ScrollOffset::new(0, 1),
            "onto the wrapped row of line 0, not its first row"
        );
        assert_eq!(
            view.scroll(Direction::Backward, 1),
            1,
            "line 0 has a row left"
        );
        assert_eq!(
            view.viewport.wrap().scroll_offset(),
            ScrollOffset::new(0, 0),
            "then up through line 0's rows"
        );
        assert_eq!(view.scroll(Direction::Backward, 1), 0, "already at the top");
    }

    #[test]
    fn scrolls_many_rows_at_once() {
        // Width 4: line 0 is "aaaa", "↩ a\n", then one row per line
        let mut view = tests::text_box("aaaaa\nb\nc\nd\ne", 4, 3);
        assert_eq!(
            view.scroll(Direction::Forward, 3),
            2,
            "stops at the last row"
        );
        assert_eq!(
            view.viewport.wrap().scroll_offset(),
            ScrollOffset::new(1, 0),
            "past both rows of line 0"
        );
        assert_eq!(
            view.scroll(Direction::Backward, 5),
            2,
            "stops at the top of the buffer"
        );
        assert_eq!(
            view.viewport.wrap().scroll_offset(),
            ScrollOffset::new(0, 0)
        );
    }

    #[test]
    fn jumps_into_the_middle_of_a_tall_line() {
        // One line far taller than the screen, the end of it can still be reached
        let mut view = tests::text_box(&"a".repeat(200), 4, 5);
        view.move_selection(Motion::File(Direction::Forward), false);
        assert_eq!(view.selection.head(), 199, "the head is not pulled back");
        let pos = view
            .viewport
            .wrap()
            .find_byte(view.selection.head())
            .unwrap();
        assert_eq!(pos.row, 2, "on the bottom margin");
    }
}
