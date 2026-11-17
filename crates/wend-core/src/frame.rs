mod area;
pub use area::{Area, Rect};
mod grapheme;
pub use grapheme::Grapheme;
mod layer;
pub use layer::Layer;
mod style;
pub use style::{Content, Style};

/// Holds what to draw in one terminal cell.
#[derive(Debug, Clone, Copy, Default)]
pub struct Cell {
    pub grapheme: Grapheme,
    pub style: Style,
    /// Stamped by the `Canvas` the cell is drawn through, so an element never picks it.
    pub layer: Layer,
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
    cells: Vec<Cell>,
    cursor: Option<CellPosition>,
    /// The layer the cursor was put on, so a cursor on a higher layer is kept.
    cursor_layer: Layer,
}

impl Frame {
    /// Creates a frame of blank cells.
    #[must_use]
    pub fn new(area: Area, cursor: Option<CellPosition>) -> Self {
        Self {
            area,
            cells: vec![Cell::default(); area.len()],
            cursor,
            cursor_layer: Layer::BOTTOM,
        }
    }

    /// Returns where the cursor is, none when the area is empty and can't show it.
    #[must_use]
    pub fn cursor(&self) -> Option<CellPosition> {
        self.cursor
    }

    /// Puts the cursor at `pos` unless it is already on a higher layer than `layer`.
    pub(crate) fn put_cursor(&mut self, pos: CellPosition, layer: Layer) {
        if self.cursor.is_none() || layer >= self.cursor_layer {
            self.cursor = Some(pos);
            self.cursor_layer = layer;
        }
    }

    /// Blanks every cell and takes the cursor away, so a frame that is drawn again starts with
    /// nothing from the last draw, on any layer.
    pub(crate) fn clear(&mut self) {
        self.cells.fill(Cell::default());
        self.cursor = None;
        self.cursor_layer = Layer::BOTTOM;
    }

    pub fn iter(&self) -> impl Iterator<Item = (CellPosition, &Cell)> {
        let Area { cols, rows } = self.area;
        (0..rows)
            .flat_map(move |row| (0..cols).map(move |col| CellPosition::new(col, row)))
            .zip(self.cells.iter())
    }

    /// Fits the frame to `area` and blanks every cell, since at a new width the old cells
    /// would land on other positions.
    pub fn resize(&mut self, area: Area) {
        self.area = area;
        self.cells.clear();
        self.cells.resize(area.len(), Cell::default());
    }

    #[must_use]
    pub fn area(&self) -> Area {
        self.area
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

    /// Writes `cell` unless the cell at `pos` is on a higher layer than it.
    ///
    /// Replacing half of a wide grapheme blanks the other half, because a terminal that is
    /// told to print into half of one blanks all of it.
    ///
    /// # Panics
    ///
    /// If `pos` is outside the frame's area.
    pub fn write(&mut self, pos: CellPosition, cell: Cell) {
        let index = self.index(pos);
        let replaced = self.cells[index];
        if replaced.layer > cell.layer {
            return;
        }
        // The cell to the right is covered only while the one at `index` is the wide head
        if usize::from(pos.col) + 1 < self.area.width() && self.cells[index + 1].covered {
            self.cells[index + 1].covered = false;
        }
        // The head of a wide grapheme is left without its other half, unless this cell is
        // the other half of a wide grapheme written in turn
        if replaced.covered && !cell.covered && pos.col > 0 {
            self.cells[index - 1] = Cell {
                grapheme: Grapheme::blank(),
                layer: replaced.layer,
                ..Cell::default()
            };
        }
        self.cells[index] = cell;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_stays_37_bytes() {
        assert_eq!(
            size_of::<Cell>(),
            37,
            "32 for the grapheme, 3 for the style, one each for the layer and covered, and the \
             frame holds one per terminal cell"
        );
    }

    fn cell(c: char, layer: Layer) -> Cell {
        Cell {
            grapheme: Grapheme::from_char(c),
            layer,
            ..Cell::default()
        }
    }

    fn wide_head(c: char) -> Cell {
        cell(c, Layer::BOTTOM)
    }

    fn tail() -> Cell {
        Cell {
            grapheme: Grapheme::blank(),
            covered: true,
            ..Cell::default()
        }
    }

    #[test]
    fn a_cell_does_not_replace_one_on_a_higher_layer() {
        let mut frame = Frame::new(Area::new(1, 1), None);
        let pos = CellPosition::new(0, 0);

        frame.write(pos, cell('a', Layer::BOTTOM.lift(1)));
        frame.write(pos, cell('b', Layer::BOTTOM));
        assert_eq!(
            frame.get(pos).grapheme.as_str_sanitize(),
            "a",
            "the lower loses"
        );

        frame.write(pos, cell('c', Layer::BOTTOM.lift(1)));
        assert_eq!(
            frame.get(pos).grapheme.as_str_sanitize(),
            "c",
            "the same layer wins"
        );

        frame.write(pos, cell('d', Layer::BOTTOM.lift(2)));
        assert_eq!(
            frame.get(pos).grapheme.as_str_sanitize(),
            "d",
            "the higher wins"
        );
    }

    #[test]
    fn replacing_the_head_of_a_wide_grapheme_uncovers_its_other_half() {
        let mut frame = Frame::new(Area::new(3, 1), None);
        frame.write(CellPosition::new(0, 0), wide_head('漢'));
        frame.write(CellPosition::new(1, 0), tail());

        frame.write(CellPosition::new(0, 0), cell('a', Layer::BOTTOM.lift(1)));

        assert!(
            !frame.get(CellPosition::new(1, 0)).covered,
            "nothing is left to cover, so the terminal prints a blank there"
        );
    }

    #[test]
    fn replacing_the_other_half_of_a_wide_grapheme_blanks_its_head() {
        let mut frame = Frame::new(Area::new(3, 1), None);
        frame.write(CellPosition::new(0, 0), wide_head('漢'));
        frame.write(CellPosition::new(1, 0), tail());

        frame.write(CellPosition::new(1, 0), cell('a', Layer::BOTTOM.lift(1)));

        assert_eq!(
            frame
                .get(CellPosition::new(0, 0))
                .grapheme
                .as_str_sanitize(),
            " ",
            "half a wide grapheme is not printed"
        );
        assert_eq!(
            frame
                .get(CellPosition::new(1, 0))
                .grapheme
                .as_str_sanitize(),
            "a"
        );
    }

    #[test]
    fn writing_a_wide_grapheme_in_order_keeps_both_halves() {
        let mut frame = Frame::new(Area::new(3, 1), None);

        frame.write(CellPosition::new(0, 0), wide_head('漢'));
        frame.write(CellPosition::new(1, 0), tail());

        assert_eq!(
            frame
                .get(CellPosition::new(0, 0))
                .grapheme
                .as_str_sanitize(),
            "漢"
        );
        assert!(frame.get(CellPosition::new(1, 0)).covered, "its other half");
    }

    #[test]
    fn clearing_takes_back_the_cells_and_the_cursor() {
        let mut frame = Frame::new(Area::new(2, 1), None);
        frame.write(CellPosition::new(0, 0), cell('a', Layer::BOTTOM.lift(3)));
        frame.put_cursor(CellPosition::new(1, 0), Layer::BOTTOM.lift(3));

        frame.clear();
        frame.put_cursor(CellPosition::new(0, 0), Layer::BOTTOM);
        frame.write(CellPosition::new(0, 0), cell('b', Layer::BOTTOM));

        assert_eq!(
            frame
                .get(CellPosition::new(0, 0))
                .grapheme
                .as_str_sanitize(),
            "b"
        );
        assert_eq!(
            frame.cursor(),
            Some(CellPosition::new(0, 0)),
            "the layers from before don't block the new draw"
        );
    }

    #[test]
    fn resize_blanks_every_cell() {
        let mut frame = Frame::new(Area::new(4, 2), None);
        frame.write(
            CellPosition::new(3, 0),
            Cell {
                style: Style {
                    selected: true,
                    ..Style::default()
                },
                ..Cell::default()
            },
        );
        frame.resize(Area::new(2, 4));

        assert_eq!(frame.area(), Area::new(2, 4));
        assert!(
            frame.iter().all(|(_, cell)| !cell.style.selected),
            "nothing from the old width shows up at another position"
        );
    }
}
