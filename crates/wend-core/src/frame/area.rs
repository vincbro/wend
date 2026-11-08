/// Converts a position inside an `Area` to a terminal coordinate.
///
/// # Panics
///
/// If `value` doesn't fit in a `u16`, which it always does inside an `Area`.
#[must_use]
pub(crate) fn terminal_coordinate(value: usize) -> u16 {
    u16::try_from(value).expect("an area is measured in u16, so positions inside it fit")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Area {
    pub cols: u16,
    pub rows: u16,
}

impl Area {
    #[must_use]
    pub fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.width() * self.height()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn width(&self) -> usize {
        usize::from(self.cols)
    }

    #[must_use]
    pub fn height(&self) -> usize {
        usize::from(self.rows)
    }
}

/// Marks out where in a frame a view is allowed to render.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub col: u16,
    pub row: u16,
    pub area: Area,
}

impl Rect {
    #[must_use]
    pub fn new(col: u16, row: u16, area: Area) -> Self {
        Self { col, row, area }
    }

    /// Splits off the left `cols` columns, which is all of the rect when it is narrower. The
    /// second rect is what is left to the right of them.
    #[must_use]
    pub fn split_left(&self, cols: u16) -> (Self, Self) {
        let left = Self::new(
            self.col,
            self.row,
            Area::new(self.area.cols.min(cols), self.area.rows),
        );
        // A rect narrower than `cols` has nothing left, and starts where it ends
        let right = Self::new(
            self.col + cols.min(self.area.cols),
            self.row,
            Area::new(self.area.cols.saturating_sub(cols), self.area.rows),
        );

        (left, right)
    }

    /// Leaves out the right `cols` columns.
    #[must_use]
    pub fn pad_right(&self, cols: u16) -> Self {
        Self::new(
            self.col,
            self.row,
            Area::new(self.area.cols.saturating_sub(cols), self.area.rows),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_left_cuts_the_columns_off_the_front() {
        let (left, right) = Rect::new(2, 1, Area::new(10, 3)).split_left(4);

        assert_eq!((left.col, left.area), (2, Area::new(4, 3)), "the front");
        assert_eq!((right.col, right.area), (6, Area::new(6, 3)), "the rest");
    }

    #[test]
    fn split_left_of_a_narrower_rect_leaves_nothing_on_the_right() {
        let (left, right) = Rect::new(2, 1, Area::new(3, 3)).split_left(4);

        assert_eq!(left.area, Area::new(3, 3), "all of the rect");
        assert_eq!(
            (right.col, right.area.cols),
            (5, 0),
            "nothing, starting where the rect ends"
        );
    }

    #[test]
    fn pad_right_leaves_out_the_right_columns() {
        let rect = Rect::new(2, 1, Area::new(10, 3));

        assert_eq!(
            rect.pad_right(4).area,
            Area::new(6, 3),
            "four columns fewer"
        );
        assert_eq!(rect.pad_right(4).col, 2, "the left edge stays");
        assert_eq!(
            rect.pad_right(40).area,
            Area::new(0, 3),
            "padding past the width leaves no columns"
        );
    }
}
