mod area;
pub(crate) use area::terminal_coordinate;
pub use area::{Area, Rect};
mod grapheme;
pub use grapheme::Grapheme;

/// Holds what to draw in one terminal cell.
#[derive(Debug, Clone, Copy, Default)]
pub struct Cell {
    pub grapheme: Grapheme,
    /// Set on every cell of the grapheme under the head, while `Frame::cursor` is the one
    /// position where that grapheme starts.
    pub cursor: bool,
    pub selected: bool,
    /// Drawn by the wide grapheme to its left, so there is nothing to print. Printing
    /// into half of a wide grapheme makes the terminal blank all of it.
    pub covered: bool,
}

/// Locates a cell in the frame, which covers the whole terminal, not just one view.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CellPosition {
    pub col: u16,
    pub row: u16,
}

impl CellPosition {
    #[must_use]
    pub fn new(col: u16, row: u16) -> Self {
        Self { col, row }
    }
}

/// Holds what the editor shows, one cell per terminal cell.
///
/// The cells are stored row by row and there are always exactly `area.len()` of them,
/// which every lookup by position relies on.
#[derive(Debug, Clone)]
pub struct Frame {
    area: Area,
    cells: Box<[Cell]>,
    cursor: Option<CellPosition>,
}

impl Frame {
    /// Creates a frame of blank cells.
    #[must_use]
    pub fn new(area: Area, cursor: Option<CellPosition>) -> Self {
        Self {
            area,
            cells: vec![Cell::default(); area.len()].into_boxed_slice(),
            cursor,
        }
    }

    /// Returns where the cursor is, none when the area is empty and can't show it.
    #[must_use]
    pub fn cursor(&self) -> Option<CellPosition> {
        self.cursor
    }

    pub fn iter(&self) -> impl Iterator<Item = (CellPosition, &Cell)> {
        let Area { cols, rows } = self.area;
        (0..rows)
            .flat_map(move |row| (0..cols).map(move |col| CellPosition::new(col, row)))
            .zip(self.cells.iter())
    }

    fn index(&self, pos: CellPosition) -> usize {
        usize::from(pos.col) + usize::from(pos.row) * self.area.width()
    }

    /// # Panics
    ///
    /// If `pos` is outside the frame's area.
    #[must_use]
    pub fn get(&self, pos: CellPosition) -> &Cell {
        &self.cells[self.index(pos)]
    }

    /// # Panics
    ///
    /// If `pos` is outside the frame's area.
    pub fn write(&mut self, pos: CellPosition, cell: Cell) {
        self.cells[self.index(pos)] = cell;
    }

    /// # Panics
    ///
    /// If `row` is outside the frame's area.
    #[must_use]
    pub fn row(&self, row: u16) -> &[Cell] {
        let start = self.index(CellPosition::new(0, row));
        &self.cells[start..start + self.area.width()]
    }
}
