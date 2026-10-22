use crate::area::Area;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cell {
    pub content: char,
    pub cursor: bool,
    pub z_index: u8,
}

impl Cell {
    #[must_use]
    pub fn new(content: char, cursor: bool, z_index: u8) -> Self {
        Self {
            content,
            cursor,
            z_index,
        }
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            content: ' ',
            cursor: false,
            z_index: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub area: Area,
    pub cells: Box<[Cell]>,
}

impl Frame {
    #[must_use]
    pub fn new(area: Area) -> Self {
        Frame {
            area,
            cells: vec![Cell::default(); area.len() as usize].into_boxed_slice(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = ((u16, u16), &Cell)> {
        let Area { cols, rows } = self.area;
        (0..rows)
            .flat_map(move |row| (0..cols).map(move |col| (col, row)))
            .zip(self.cells.iter())
    }

    #[must_use]
    pub fn get(&self, x: u16, y: u16) -> &Cell {
        let idx = u32::from(x) + u32::from(y) * u32::from(self.area.cols);
        &self.cells[idx as usize]
    }

    pub fn get_mut(&mut self, x: u16, y: u16) -> &mut Cell {
        let idx = u32::from(x) + u32::from(y) * u32::from(self.area.cols);
        &mut self.cells[idx as usize]
    }

    pub fn write(&mut self, x: u16, y: u16, cell: Cell) {
        let idx = u32::from(x) + u32::from(y) * u32::from(self.area.cols);
        self.cells[idx as usize] = cell;
    }

    #[must_use]
    pub fn get_row(&self, row: u16) -> &[Cell] {
        let start = u32::from(row) * u32::from(self.area.cols);
        let end = start + u32::from(self.area.cols);
        let range = start as usize..end as usize;
        &self.cells[range]
    }
}
