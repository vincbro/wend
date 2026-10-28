use crate::{area::Area, editor::Mode, grapheme::Grapheme};

#[derive(Debug, Clone, Copy, Default)]
pub struct Cell {
    pub grapheme: Grapheme,
    pub cursor: bool,
    pub selected: bool,
    /// Drawn by the wide grapheme to its left, so there is nothing to print. Printing
    /// into half of a wide grapheme makes the terminal blank all of it.
    pub covered: bool,
    pub z_index: u8,
}

/// A cell in the frame, which covers the whole terminal, not just one view.
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

/// What the editor shows, one cell per terminal cell.
///
/// The cells are stored row by row and there are always exactly `area.len()` of them,
/// which every lookup by position relies on.
#[derive(Debug, Clone)]
pub struct Frame {
    area: Area,
    cells: Box<[Cell]>,
    mode: Mode,
    cursor: CellPosition,
}

impl Frame {
    /// A frame of blank cells.
    #[must_use]
    pub fn new(area: Area, mode: Mode, cursor: CellPosition) -> Self {
        Self {
            area,
            cells: vec![Cell::default(); area.len()].into_boxed_slice(),
            mode,
            cursor,
        }
    }

    #[must_use]
    pub fn area(&self) -> Area {
        self.area
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    #[must_use]
    pub fn cursor(&self) -> CellPosition {
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
    pub fn get_mut(&mut self, pos: CellPosition) -> &mut Cell {
        &mut self.cells[self.index(pos)]
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
        let start = u32::from(row) * u32::from(self.area.cols);
        let end = start + u32::from(self.area.cols);
        let range = start as usize..end as usize;
        &self.cells[range]
    }
}
