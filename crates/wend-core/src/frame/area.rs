use super::CellPosition;

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

/// Marks out where in a frame an element is allowed to render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    /// Splits off the right `cols` columns, which is all of the rect when it is narrower. The
    /// second rect is what is left to the left of them.
    #[must_use]
    pub fn split_right(&self, cols: u16) -> (Self, Self) {
        let (rest, right) = self.split_left(self.area.cols.saturating_sub(cols));
        (right, rest)
    }

    /// Splits off the top `rows` rows, which is all of the rect when it is shorter. The second
    /// rect is what is left below them.
    #[must_use]
    pub fn split_top(&self, rows: u16) -> (Self, Self) {
        let top = Self::new(
            self.col,
            self.row,
            Area::new(self.area.cols, self.area.rows.min(rows)),
        );
        // A rect shorter than `rows` has nothing left, and starts where it ends
        let rest = Self::new(
            self.col,
            self.row + rows.min(self.area.rows),
            Area::new(self.area.cols, self.area.rows.saturating_sub(rows)),
        );

        (top, rest)
    }

    /// Splits off the bottom `rows` rows, which is all of the rect when it is shorter. The
    /// second rect is what is left above them.
    #[must_use]
    pub fn split_bottom(&self, rows: u16) -> (Self, Self) {
        let (rest, bottom) = self.split_top(self.area.rows.saturating_sub(rows));
        (bottom, rest)
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

    /// Returns the part of the rect that is also in `other`, which has no cells when they don't
    /// touch.
    #[must_use]
    pub fn intersection(&self, other: Self) -> Self {
        let col = self.col.max(other.col);
        let row = self.row.max(other.row);
        let right = (self.col + self.area.cols).min(other.col + other.area.cols);
        let bottom = (self.row + self.area.rows).min(other.row + other.area.rows);
        // Rects that don't touch end before the other starts
        Self::new(
            col,
            row,
            Area::new(right.saturating_sub(col), bottom.saturating_sub(row)),
        )
    }

    /// Returns whether the cell at `pos` is inside the rect.
    #[must_use]
    pub fn contains(&self, pos: CellPosition) -> bool {
        // Widened, because a rect can end at the very edge of the u16 columns
        let inside = |start: u16, len: u16, at: u16| {
            u32::from(at) >= u32::from(start) && u32::from(at) < u32::from(start) + u32::from(len)
        };
        inside(self.col, self.area.cols, pos.col) && inside(self.row, self.area.rows, pos.row)
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
    fn split_right_cuts_the_columns_off_the_back() {
        let (right, left) = Rect::new(2, 1, Area::new(10, 3)).split_right(4);

        assert_eq!((right.col, right.area), (8, Area::new(4, 3)), "the back");
        assert_eq!((left.col, left.area), (2, Area::new(6, 3)), "the rest");
    }

    #[test]
    fn split_right_of_a_narrower_rect_takes_all_of_it() {
        let (right, left) = Rect::new(2, 1, Area::new(3, 3)).split_right(4);

        assert_eq!((right.col, right.area), (2, Area::new(3, 3)), "all of it");
        assert_eq!(left.area.cols, 0, "nothing is left");
    }

    #[test]
    fn split_top_cuts_the_rows_off_the_front() {
        let (top, rest) = Rect::new(2, 1, Area::new(10, 5)).split_top(2);

        assert_eq!((top.row, top.area), (1, Area::new(10, 2)), "the top");
        assert_eq!((rest.row, rest.area), (3, Area::new(10, 3)), "the rest");
    }

    #[test]
    fn split_top_of_a_shorter_rect_leaves_nothing_below() {
        let (top, rest) = Rect::new(2, 1, Area::new(10, 2)).split_top(5);

        assert_eq!(top.area, Area::new(10, 2), "all of the rect");
        assert_eq!(
            (rest.row, rest.area.rows),
            (3, 0),
            "nothing, starting where the rect ends"
        );
    }

    #[test]
    fn split_bottom_cuts_the_rows_off_the_back() {
        let (bottom, rest) = Rect::new(2, 1, Area::new(10, 5)).split_bottom(1);

        assert_eq!((bottom.row, bottom.area), (5, Area::new(10, 1)), "the last");
        assert_eq!((rest.row, rest.area), (1, Area::new(10, 4)), "the rest");
    }

    #[test]
    fn split_bottom_of_a_shorter_rect_takes_all_of_it() {
        let (bottom, rest) = Rect::new(2, 1, Area::new(10, 2)).split_bottom(5);

        assert_eq!(
            (bottom.row, bottom.area),
            (1, Area::new(10, 2)),
            "all of it"
        );
        assert_eq!(rest.area.rows, 0, "nothing is left");
    }

    #[test]
    fn intersection_is_the_cells_in_both() {
        let rect = Rect::new(2, 1, Area::new(6, 4));

        assert_eq!(
            rect.intersection(Rect::new(4, 0, Area::new(10, 3))),
            Rect::new(4, 1, Area::new(4, 2)),
            "overlapping at a corner"
        );
        assert_eq!(
            rect.intersection(Rect::new(0, 0, Area::new(20, 20))),
            rect,
            "inside the other"
        );
        assert!(
            rect.intersection(Rect::new(10, 1, Area::new(3, 4)))
                .area
                .is_empty(),
            "side by side"
        );
    }

    #[test]
    fn contains_tells_the_inside_from_the_edges() {
        let rect = Rect::new(2, 1, Area::new(3, 2));

        assert!(rect.contains(CellPosition::new(2, 1)), "the first cell");
        assert!(rect.contains(CellPosition::new(4, 2)), "the last cell");
        assert!(!rect.contains(CellPosition::new(5, 1)), "past the right");
        assert!(!rect.contains(CellPosition::new(2, 3)), "past the bottom");
        assert!(!rect.contains(CellPosition::new(1, 1)), "before the left");
        assert!(!rect.contains(CellPosition::new(2, 0)), "above the top");
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
