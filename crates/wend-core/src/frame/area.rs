/// Converts a position inside an `Area` to a terminal coordinate.
///
/// # Panics
///
/// If `value` doesn't fit in a `u16`, which it always does inside an `Area`.
#[must_use]
pub(crate) fn terminal_coordinate(value: usize) -> u16 {
    u16::try_from(value).expect("an area is measured in u16, so positions inside it fit")
}

#[derive(Debug, Clone, Copy)]
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
}
