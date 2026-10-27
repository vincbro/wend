use crate::{area::Area, editor::Mode, grapheme::Grapheme};

#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub grapheme: Grapheme,
    pub cursor: bool,
    pub selected: bool,
    pub z_index: u8,
}

impl Cell {
    #[must_use]
    pub fn new(grapheme: Grapheme, cursor: bool, selected: bool, z_index: u8) -> Self {
        Self {
            grapheme,
            cursor,
            selected,
            z_index,
        }
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            grapheme: Grapheme::default(),
            cursor: false,
            selected: false,
            z_index: 0,
        }
    }
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

#[derive(Debug, Clone)]
pub struct Frame {
    pub area: Area,
    pub cells: Box<[Cell]>,
    pub mode: Mode,
    pub cursor: CellPosition,
}

impl Frame {
    #[must_use]
    pub fn new(area: Area) -> Self {
        Frame {
            area,
            cells: vec![Cell::default(); area.len_u32() as usize].into_boxed_slice(),
            mode: Mode::Normal,
            cursor: CellPosition::default(),
        }
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

    #[must_use]
    pub fn get(&self, pos: CellPosition) -> &Cell {
        &self.cells[self.index(pos)]
    }

    pub fn get_mut(&mut self, pos: CellPosition) -> &mut Cell {
        &mut self.cells[self.index(pos)]
    }

    pub fn write(&mut self, pos: CellPosition, cell: Cell) {
        self.cells[self.index(pos)] = cell;
    }

    #[must_use]
    pub fn get_row(&self, row: u16) -> &[Cell] {
        let start = u32::from(row) * u32::from(self.area.cols);
        let end = start + u32::from(self.area.cols);
        let range = start as usize..end as usize;
        &self.cells[range]
    }
}
