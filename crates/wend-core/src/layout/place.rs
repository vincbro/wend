use super::Size;
use crate::frame::{Area, Rect};

/// Which side of the space a strip is taken from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// Where something smaller than its space sits along one axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

impl Align {
    /// Returns how many cells in from the start something sits, with `spare` cells to share.
    fn offset(self, spare: u16) -> u16 {
        match self {
            Self::Start => 0,
            Self::Center => spare / 2,
            Self::End => spare,
        }
    }
}

/// A size on both axes, and where that size sits when it is smaller than the space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fit {
    pub cols: Size,
    pub rows: Size,
    pub horizontal: Align,
    pub vertical: Align,
}

impl Fit {
    /// All of the space.
    pub const FILL: Self = Self {
        cols: Size::Fill,
        rows: Size::Fill,
        horizontal: Align::Start,
        vertical: Align::Start,
    };

    #[must_use]
    pub fn centered(cols: Size, rows: Size) -> Self {
        Self {
            cols,
            rows,
            horizontal: Align::Center,
            vertical: Align::Center,
        }
    }

    /// Returns the rect inside `within` this fit works out to.
    #[must_use]
    fn place(self, within: Rect) -> Rect {
        let cols = self.cols.resolve(within.area.cols);
        let rows = self.rows.resolve(within.area.rows);
        // A size never takes more than the space, so these can't go below zero
        Rect::new(
            within.col + self.horizontal.offset(within.area.cols - cols),
            within.row + self.vertical.offset(within.area.rows - rows),
            Area::new(cols, rows),
        )
    }
}

/// Where an element wants to be inside the space its parent allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Takes a strip off the free space, so the siblings after it get what is left.
    Dock { edge: Edge, size: Size },
    /// Sits inside the free space, which it leaves as it was.
    Fit(Fit),
    /// Sits inside the parent's whole rect, one layer above it, and leaves the free space as
    /// it was.
    Float(Fit),
}

impl Layout {
    pub const FILL: Self = Self::Fit(Fit::FILL);

    /// Returns where to draw, and the free space left for the next sibling. `free` is what
    /// the siblings before it left, `bounds` is the parent's whole rect.
    #[must_use]
    pub(crate) fn place(self, free: Rect, bounds: Rect) -> (Rect, Rect) {
        match self {
            Self::Dock { edge, size } => match edge {
                Edge::Top => free.split_top(size.resolve(free.area.rows)),
                Edge::Bottom => free.split_bottom(size.resolve(free.area.rows)),
                Edge::Left => free.split_left(size.resolve(free.area.cols)),
                Edge::Right => free.split_right(size.resolve(free.area.cols)),
            },
            Self::Fit(fit) => (fit.place(free), free),
            Self::Float(fit) => (fit.place(bounds), free),
        }
    }

    /// Returns how many layers above its parent this draws.
    #[must_use]
    pub(crate) fn lift(self) -> u8 {
        u8::from(matches!(self, Self::Float(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> Rect {
        Rect::new(2, 1, Area::new(20, 10))
    }

    fn at(rect: Rect) -> (u16, u16, Area) {
        (rect.col, rect.row, rect.area)
    }

    #[test]
    fn a_fit_aligns_inside_its_space() {
        let fixed = |horizontal, vertical| Fit {
            cols: Size::Fixed(6),
            rows: Size::Fixed(2),
            horizontal,
            vertical,
        };

        assert_eq!(
            at(fixed(Align::Start, Align::Start).place(rect())),
            (2, 1, Area::new(6, 2)),
            "the top left corner"
        );
        assert_eq!(
            at(fixed(Align::Center, Align::Center).place(rect())),
            (9, 5, Area::new(6, 2)),
            "seven columns and four rows spare, shared evenly"
        );
        assert_eq!(
            at(fixed(Align::End, Align::End).place(rect())),
            (16, 9, Area::new(6, 2)),
            "the bottom right corner"
        );
    }

    #[test]
    fn a_fit_larger_than_its_space_takes_the_space() {
        let fit = Fit::centered(Size::Fixed(50), Size::Fill);

        assert_eq!(at(fit.place(rect())), at(rect()), "nothing to center in");
    }

    #[test]
    fn fill_is_all_of_the_free_space_and_leaves_it_as_it_was() {
        let (placed, free) = Layout::FILL.place(rect(), rect());

        assert_eq!(placed, rect(), "all of the free space");
        assert_eq!(free, rect(), "a sibling after it overlaps it");
    }

    #[test]
    fn a_dock_takes_a_strip_and_leaves_the_rest() {
        let dock = |edge| Layout::Dock {
            edge,
            size: Size::Fixed(3),
        };

        let (top, rest) = dock(Edge::Top).place(rect(), rect());
        assert_eq!(
            (at(top), at(rest)),
            ((2, 1, Area::new(20, 3)), (2, 4, Area::new(20, 7))),
            "a strip off the top, the rest beside or under it"
        );
        let (bottom, rest) = dock(Edge::Bottom).place(rect(), rect());
        assert_eq!(
            (at(bottom), at(rest)),
            ((2, 8, Area::new(20, 3)), (2, 1, Area::new(20, 7))),
            "a strip off the bottom, the rest beside or under it"
        );
        let (left, rest) = dock(Edge::Left).place(rect(), rect());
        assert_eq!(
            (at(left), at(rest)),
            ((2, 1, Area::new(3, 10)), (5, 1, Area::new(17, 10))),
            "a strip off the left, the rest beside or under it"
        );
        let (right, rest) = dock(Edge::Right).place(rect(), rect());
        assert_eq!(
            (at(right), at(rest)),
            ((19, 1, Area::new(3, 10)), (2, 1, Area::new(17, 10))),
            "a strip off the right, the rest beside or under it"
        );
    }

    #[test]
    fn a_dock_is_sized_by_the_axis_it_takes_from() {
        let dock = Layout::Dock {
            edge: Edge::Bottom,
            size: Size::Percent(50),
        };

        let (strip, _) = dock.place(rect(), rect());

        assert_eq!(
            strip.area,
            Area::new(20, 5),
            "half of the rows, not the columns"
        );
    }

    #[test]
    fn the_second_dock_takes_from_what_the_first_left() {
        let bar = Layout::Dock {
            edge: Edge::Bottom,
            size: Size::Fixed(1),
        };
        let (_, free) = bar.place(rect(), rect());

        let (second, _) = bar.place(free, rect());

        assert_eq!(
            second.row, 9,
            "one row above the first, which is on the last row, 10"
        );
    }

    #[test]
    fn a_float_sits_in_the_whole_rect_and_leaves_the_free_space() {
        let (_, free) = Layout::Dock {
            edge: Edge::Bottom,
            size: Size::Fixed(1),
        }
        .place(rect(), rect());
        let float = Layout::Float(Fit::centered(Size::Fixed(6), Size::Fixed(2)));

        let (placed, left) = float.place(free, rect());

        assert_eq!(
            at(placed),
            (9, 5, Area::new(6, 2)),
            "centered in the parent, the docked row included"
        );
        assert_eq!(left, free, "the siblings after it don't make room for it");
    }

    #[test]
    fn only_a_float_lifts_a_layer() {
        let dock = Layout::Dock {
            edge: Edge::Top,
            size: Size::Fill,
        };

        assert_eq!(Layout::FILL.lift(), 0, "a fit stays on the layer");
        assert_eq!(dock.lift(), 0, "so does a dock");
        assert_eq!(Layout::Float(Fit::FILL).lift(), 1, "a float goes above");
    }
}
