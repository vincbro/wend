use super::Element;
use crate::frame::{Cell, CellPosition, Frame, Layer, Rect};

/// Where an element draws: a frame, cut down to the rect the element was given and on the
/// layer it belongs to.
///
/// The canvas stamps its layer on every cell and drops what falls outside its rect, so an
/// element can't draw outside its space or on the wrong layer.
#[derive(Debug)]
pub struct Canvas<'frame> {
    frame: &'frame mut Frame,
    clip: Rect,
    layer: Layer,
}

impl<'frame> Canvas<'frame> {
    /// Starts a draw of the whole of `frame`, which is blanked first.
    #[must_use]
    pub fn root(frame: &'frame mut Frame) -> Self {
        frame.clear();
        Self {
            clip: Rect::new(0, 0, frame.area()),
            frame,
            layer: Layer::BOTTOM,
        }
    }

    /// Fits `child` into `free`, draws it and returns the free space it leaves.
    ///
    /// Docking takes space in the order it is asked for, so docked children go before the
    /// one that fills what is left.
    pub fn child(&mut self, child: &mut impl Element, free: Rect) -> Rect {
        let layout = child.layout();
        let (placed, left_over) = layout.place(free, self.clip);
        let mut canvas = Canvas {
            frame: &mut *self.frame,
            clip: placed.intersection(self.clip),
            layer: self.layer.lift(layout.lift()),
        };
        child.render(placed, &mut canvas);
        left_over
    }

    /// Writes `cell` on this canvas's layer. Nothing is written outside the canvas's rect, or
    /// over a cell on a higher layer.
    pub fn write(&mut self, pos: CellPosition, mut cell: Cell) {
        if !self.clip.contains(pos) {
            return;
        }
        cell.layer = self.layer;
        self.frame.write(pos, cell);
    }

    /// Puts the cursor at `pos`, unless that is outside the canvas's rect or the cursor is on a
    /// higher layer already.
    pub fn set_cursor(&mut self, pos: CellPosition) {
        if self.clip.contains(pos) {
            self.frame.put_cursor(pos, self.layer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        frame::{Area, Grapheme},
        layout::{Edge, Fit, Layout, Size},
    };

    /// Draws `letter` over all of its rect, in the layout it was made with.
    struct Fill {
        layout: Layout,
        letter: char,
    }

    impl Element for Fill {
        fn layout(&self) -> Layout {
            self.layout
        }

        fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
            let rows = rect.row..rect.row + rect.area.rows;
            let cells = rows.flat_map(|row| {
                (rect.col..rect.col + rect.area.cols).map(move |col| CellPosition::new(col, row))
            });
            for pos in cells {
                canvas.write(pos, letter(self.letter));
            }
        }
    }

    fn letter(c: char) -> Cell {
        Cell {
            grapheme: Grapheme::from_char(c),
            ..Cell::default()
        }
    }

    fn text(frame: &Frame, row: u16) -> String {
        frame
            .row(row)
            .iter()
            .map(|cell| cell.grapheme.as_str_sanitize())
            .collect()
    }

    fn fill(layout: Layout, letter: char) -> Fill {
        Fill { layout, letter }
    }

    fn bar(edge: Edge, letter: char) -> Fill {
        fill(
            Layout::Dock {
                edge,
                size: Size::Fixed(1),
            },
            letter,
        )
    }

    #[test]
    fn a_dock_and_a_fill_share_the_frame() {
        let mut frame = Frame::new(Area::new(3, 3), None);
        let mut canvas = Canvas::root(&mut frame);
        let all = Rect::new(0, 0, Area::new(3, 3));

        let free = canvas.child(&mut bar(Edge::Bottom, 'b'), all);
        canvas.child(&mut fill(Layout::FILL, 'f'), free);

        assert_eq!(
            [text(&frame, 0), text(&frame, 1), text(&frame, 2)],
            ["fff", "fff", "bbb"],
            "the bar's row is not the fill's"
        );
    }

    #[test]
    fn a_float_wins_over_what_is_drawn_after_it() {
        let mut frame = Frame::new(Area::new(3, 1), None);
        let mut canvas = Canvas::root(&mut frame);
        let all = Rect::new(0, 0, Area::new(3, 1));
        let float = Layout::Float(Fit::centered(Size::Fixed(1), Size::Fill));

        canvas.child(&mut fill(float, 'F'), all);
        canvas.child(&mut fill(Layout::FILL, 'x'), all);

        assert_eq!(text(&frame, 0), "xFx", "the float stays on top");
    }

    /// Writes `z` in the last column of the frame, whatever its rect is.
    struct Stray {
        layout: Layout,
    }

    impl Element for Stray {
        fn layout(&self) -> Layout {
            self.layout
        }

        fn render(&mut self, _rect: Rect, canvas: &mut Canvas<'_>) {
            canvas.write(CellPosition::new(3, 0), letter('z'));
        }
    }

    #[test]
    fn a_cell_outside_the_rect_is_dropped() {
        let mut frame = Frame::new(Area::new(4, 1), None);
        let mut canvas = Canvas::root(&mut frame);
        let all = Rect::new(0, 0, Area::new(4, 1));
        let first_two = Layout::Fit(Fit {
            cols: Size::Fixed(2),
            ..Fit::FILL
        });

        canvas.child(&mut Stray { layout: first_two }, all);
        assert_eq!(
            text(&frame, 0),
            "    ",
            "column 3 is outside the two columns"
        );

        let mut canvas = Canvas::root(&mut frame);
        canvas.child(
            &mut Stray {
                layout: Layout::FILL,
            },
            all,
        );
        assert_eq!(text(&frame, 0), "   z", "and inside all four");
    }

    #[test]
    fn starting_a_draw_blanks_the_frame_and_the_cursor() {
        let mut frame = Frame::new(Area::new(2, 1), Some(CellPosition::new(1, 0)));
        Canvas::root(&mut frame).write(CellPosition::new(0, 0), letter('a'));
        let mut canvas = Canvas::root(&mut frame);

        canvas.write(CellPosition::new(1, 0), letter('b'));

        assert_eq!(
            text(&frame, 0),
            " b",
            "nothing is left from the draw before"
        );
        assert_eq!(frame.cursor(), None, "and no cursor");
    }

    /// Puts the cursor `offset` columns into its rect.
    struct Cursor {
        layout: Layout,
        offset: u16,
    }

    impl Element for Cursor {
        fn layout(&self) -> Layout {
            self.layout
        }

        fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>) {
            canvas.set_cursor(CellPosition::new(rect.col + self.offset, rect.row));
        }
    }

    #[test]
    fn the_cursor_stays_on_the_highest_layer_that_put_it() {
        let mut frame = Frame::new(Area::new(3, 1), None);
        let mut canvas = Canvas::root(&mut frame);
        let all = Rect::new(0, 0, Area::new(3, 1));

        canvas.child(
            &mut Cursor {
                layout: Layout::Float(Fit::FILL),
                offset: 2,
            },
            all,
        );
        canvas.child(
            &mut Cursor {
                layout: Layout::FILL,
                offset: 0,
            },
            all,
        );

        assert_eq!(
            frame.cursor(),
            Some(CellPosition::new(2, 0)),
            "the float's, not the one drawn after it"
        );
    }
}
