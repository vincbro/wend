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
    pub fn len(&self) -> u32 {
        u32::from(self.cols) * u32::from(self.rows)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Where in a frame a view is allowed to render
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
