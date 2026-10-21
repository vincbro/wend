use crate::area::Area;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cell {
    pub content: char,
    pub cursor: bool,
    pub z_index: u8,
}

impl Cell {
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

pub struct Frame {
    pub area: Area,
    pub buffer: Box<[Cell]>,
}

impl Frame {
    pub fn new(area: Area) -> Self {
        Frame {
            area,
            buffer: vec![Cell::default(); area.len() as usize].into_boxed_slice(),
        }
    }

    pub fn get(&self, x: u16, y: u16) -> &Cell {
        let idx = u32::from(x) + u32::from(y) * u32::from(self.area.cols);
        &self.buffer[idx as usize]
    }

    pub fn get_mut(&mut self, x: u16, y: u16) -> &mut Cell {
        let idx = u32::from(x) + u32::from(y) * u32::from(self.area.cols);
        &mut self.buffer[idx as usize]
    }

    pub fn write(&mut self, x: u16, y: u16, cell: Cell) {
        let idx = u32::from(x) + u32::from(y) * u32::from(self.area.cols);
        self.buffer[idx as usize] = cell;
    }

    pub fn get_row<'a>(&'a self, row: u16) -> &'a [Cell] {
        let start = u32::from(row) * u32::from(self.area.cols);
        let end = start + u32::from(self.area.cols);
        let range = start as usize..end as usize;
        &self.buffer[range]
    }
}
