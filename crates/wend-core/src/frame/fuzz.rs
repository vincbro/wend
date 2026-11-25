//! Drives a frame with random calls and checks its invariants after each one (S-INVARIANTS).

use proptest::{collection, prelude::*};

use super::{Cell, CellPosition, Frame, Grapheme, Layer};
use crate::{frame::Area, text_box::fuzz as text_box_fuzz};

/// One public call on a frame, with its arguments. Positions are taken modulo the area, since a
/// position outside it is a panic the docs promise.
#[derive(Debug, Clone)]
enum Call {
    Write {
        col: u16,
        row: u16,
        character: char,
        layer: u8,
        covered: bool,
    },
    PutCursor {
        col: u16,
        row: u16,
        layer: u8,
    },
    Clear,
    Resize(Area),
}

fn call() -> impl Strategy<Value = Call> {
    prop_oneof![
        6 => (any::<u16>(), any::<u16>(), prop_oneof![Just('a'), Just('漢'), Just(' '), any::<char>()], 0..3u8, any::<bool>())
            .prop_map(|(col, row, character, layer, covered)| Call::Write { col, row, character, layer, covered }),
        2 => (any::<u16>(), any::<u16>(), 0..3u8).prop_map(|(col, row, layer)| Call::PutCursor { col, row, layer }),
        1 => Just(Call::Clear),
        2 => text_box_fuzz::area().prop_map(Call::Resize),
    ]
}

proptest! {
    #[test]
    fn no_sequence_of_calls_breaks_the_invariants(
        start in text_box_fuzz::area(),
        calls in collection::vec(call(), 0..60),
    ) {
        let mut frame = Frame::new(start, None);
        frame.check_invariants();
        for call in calls {
            match call {
                Call::Write { col, row, character, layer, covered } => {
                    let area = frame.area();
                    if area.is_empty() {
                        continue;
                    }
                    let pos = CellPosition::new(col % area.cols, row % area.rows);
                    let cell = Cell {
                        grapheme: Grapheme::from_char(character),
                        layer: Layer::BOTTOM.lift(layer),
                        covered,
                        ..Cell::default()
                    };
                    let before = frame.get(pos).layer;
                    frame.write(pos, cell);
                    prop_assert!(
                        frame.get(pos).layer >= before,
                        "a write never lowers the layer of a cell"
                    );
                    prop_assert!(
                        before > cell.layer || frame.get(pos).layer == cell.layer,
                        "a write on the same layer or a higher one lands"
                    );
                }
                Call::PutCursor { col, row, layer } => {
                    let area = frame.area();
                    if area.is_empty() {
                        continue;
                    }
                    frame.put_cursor(CellPosition::new(col % area.cols, row % area.rows), Layer::BOTTOM.lift(layer));
                }
                Call::Clear => frame.clear(),
                Call::Resize(area) => frame.resize(area),
            }
            frame.check_invariants();
            prop_assert_eq!(
                frame.iter().count(),
                frame.area().len(),
                "iter walks every cell of the area once"
            );
        }
    }
}
