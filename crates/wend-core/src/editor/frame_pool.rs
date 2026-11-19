use crate::frame::{Area, Frame};

/// The frames the editor can draw into. A frame goes out with `Output::Frame` and comes
/// back with `Input::Drawn`, so only `count` frames ever exist and none is allocated
/// after startup. An empty pool means the frontend is still drawing.
///
/// Never more than `count` frames are spare, and every one of them is as big as `area`.
/// `check_invariants` checks it.
#[derive(Debug)]
pub(super) struct FramePool {
    spares: Vec<Frame>,
    count: usize,
    area: Area,
}

impl FramePool {
    #[must_use]
    pub(super) fn new(count: usize, area: Area) -> Self {
        let pool = Self {
            area,
            count,
            spares: vec![Frame::new(area, None); count],
        };
        pool.check_invariants();
        pool
    }

    /// Returns whether every frame is out, so there is none to draw into.
    #[must_use]
    pub(super) fn is_empty(&self) -> bool {
        self.spares.is_empty()
    }

    #[must_use]
    pub(super) fn take(&mut self) -> Option<Frame> {
        self.spares.pop()
    }

    /// Takes a frame back. One more than the pool made isn't one of its own, so it is dropped.
    pub(super) fn place(&mut self, mut frame: Frame) {
        if self.spares.len() < self.count {
            if frame.area() != self.area {
                frame.resize(self.area);
            }
            self.spares.push(frame);
        }
        self.check_invariants();
    }

    pub(super) fn resize(&mut self, area: Area) {
        self.area = area;
        self.spares
            .iter_mut()
            .filter(|frame| frame.area() != area)
            .for_each(|frame| {
                frame.resize(area);
            });
        self.check_invariants();
    }

    /// Checks the rules every function has to leave the pool in. Debug builds only.
    fn check_invariants(&self) {
        if !cfg!(debug_assertions) {
            return;
        }

        assert!(
            self.spares.len() <= self.count,
            "place only takes a frame while there are fewer spares than the pool made"
        );
        assert!(
            self.spares.iter().all(|frame| frame.area() == self.area),
            "place and resize bring every spare to the pool's area"
        );
    }
}
