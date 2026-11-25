//! Drives a pool with random calls and checks its invariants after each one (S-INVARIANTS).
//! A count of the spare frames is the model the pool is compared with.

use proptest::{collection, prelude::*};

use super::FramePool;
use crate::{
    frame::{Area, Frame},
    text_box::fuzz as text_box_fuzz,
};

/// One public call on a pool, with its arguments.
#[derive(Debug, Clone)]
enum Call {
    Take,
    /// Gives back the frame taken at this position, if one is out.
    Place(usize),
    /// Gives in a frame the pool didn't make, of any size.
    Stranger(Area),
    Resize(Area),
}

fn call() -> impl Strategy<Value = Call> {
    prop_oneof![
        4 => Just(Call::Take),
        4 => any::<usize>().prop_map(Call::Place),
        1 => text_box_fuzz::area().prop_map(Call::Stranger),
        2 => text_box_fuzz::area().prop_map(Call::Resize),
    ]
}

proptest! {
    #[test]
    fn no_sequence_of_calls_breaks_the_invariants(
        count in 0..=5usize,
        start in text_box_fuzz::area(),
        calls in collection::vec(call(), 0..40),
    ) {
        let mut pool = FramePool::new(count, start);
        let mut out: Vec<Frame> = vec![];
        let mut spares = count;
        for call in calls {
            match call {
                Call::Take => {
                    let frame = pool.take();
                    prop_assert_eq!(frame.is_some(), spares > 0, "a frame is taken while a spare is left");
                    if let Some(frame) = frame {
                        spares -= 1;
                        out.push(frame);
                    }
                }
                Call::Place(index) => {
                    if out.is_empty() {
                        continue;
                    }
                    pool.place(out.swap_remove(index % out.len()));
                    spares = (spares + 1).min(count);
                }
                Call::Stranger(area) => {
                    pool.place(Frame::new(area, None));
                    spares = (spares + 1).min(count);
                }
                Call::Resize(area) => pool.resize(area),
            }
            pool.check_invariants();
            prop_assert_eq!(pool.is_empty(), spares == 0, "it is empty when no spare is left");
        }
    }
}
