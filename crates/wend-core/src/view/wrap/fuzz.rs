//! Drives a wrap with random rebuilds and checks its invariants after each one (S-INVARIANTS).

use std::num::NonZeroUsize;

use proptest::{collection, prelude::*};

use super::{Glyph, ScrollOffset, SlotPosition, Wrap};
use crate::{
    buffer::Buffer,
    frame::Area,
    view::{Direction, fuzz},
};

/// One call on a wrap, with its arguments. A line is a fraction of the lines the text has,
/// so it is always one that exists.
#[derive(Debug, Clone)]
enum Call {
    Text(String),
    Rebuild {
        cols: u16,
        rows: u16,
        line: usize,
        row: usize,
    },
    SetTabWidth(NonZeroUsize),
    LastRowOfLine {
        cols: u16,
        line: usize,
    },
}

fn call() -> impl Strategy<Value = Call> {
    // From no columns at all to wider than most lines, rows from none to a screenful
    let cols = prop_oneof![Just(0), 1..=12u16, 13..=60u16];
    let rows = 0..=8u16;
    // A row past the end of the line is clamped, so does a huge one
    let row = prop_oneof![3 => 0..4usize, 1 => Just(usize::MAX)];
    prop_oneof![
        2 => fuzz::text().prop_map(Call::Text),
        6 => (cols.clone(), rows, any::<usize>(), row)
            .prop_map(|(cols, rows, line, row)| Call::Rebuild { cols, rows, line, row }),
        1 => (1..=8usize).prop_map(|width| Call::SetTabWidth(NonZeroUsize::new(width).unwrap())),
        1 => (cols, any::<usize>()).prop_map(|(cols, line)| Call::LastRowOfLine { cols, line }),
    ]
}

/// What the lookups say about every byte the rows show.
fn check_lookups(wrap: &Wrap) {
    for row in wrap.rows() {
        for byte in row.metadata.from_byte..row.metadata.to_byte {
            let found = wrap
                .find_byte(byte)
                .and_then(|pos| wrap.glyph_at(pos))
                .map(Glyph::to_range);
            assert!(
                found.is_some_and(|range| range.contains(&byte)),
                "a byte a row shows is found in the glyph that holds it, byte {byte}"
            );
        }
    }
    for row in 0..wrap.area.height() {
        for direction in [Direction::Backward, Direction::Forward] {
            let _ = wrap.edge_glyph(row, direction);
            let _ = wrap.step(SlotPosition::new(0, row), direction);
        }
    }
}

proptest! {
    #[test]
    fn no_sequence_of_calls_breaks_the_invariants(
        initial in fuzz::text(),
        calls in collection::vec(call(), 0..30),
    ) {
        let mut buffer = Buffer::from(initial.as_str());
        let mut wrap = Wrap::new(Area::new(10, 4), ScrollOffset::default(), NonZeroUsize::new(4).unwrap());

        for call in calls {
            match call {
                Call::Text(text) => buffer = Buffer::from(text.as_str()),
                Call::Rebuild { cols, rows, line, row } => {
                    let line = line % (buffer.rope().line_len() + 1);
                    wrap.rebuild(&buffer, Area::new(cols, rows), ScrollOffset::new(line, row));
                }
                Call::SetTabWidth(width) => wrap.set_tab_width(width),
                Call::LastRowOfLine { cols, line } => {
                    let line = line % (buffer.rope().line_len() + 1);
                    let _ = wrap.last_row_of_line(&buffer, cols, line);
                }
            }
            wrap.check_invariants();
            check_lookups(&wrap);
        }
    }
}
