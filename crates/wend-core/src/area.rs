/// Converts to a terminal coordinate, clamping at `u16::MAX` instead of failing.
#[must_use]
pub(crate) fn saturating_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
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
    pub fn len_u32(&self) -> u32 {
        u32::from(self.cols) * u32::from(self.rows)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.width() * self.height()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len_u32() == 0
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
