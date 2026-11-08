use crate::frame::{Area, Frame};

/// The frames the editor can draw into. A frame goes out with `Output::Frame` and comes
/// back with `Input::Drawn`, so only `count` frames ever exist and none is allocated
/// after startup. An empty pool means the frontend is still drawing.
#[derive(Debug)]
pub(super) struct FramePool {
    spares: Vec<Frame>,
    area: Area,
}

impl FramePool {
    #[must_use]
    pub(super) fn new(count: usize, area: Area) -> Self {
        Self {
            area,
            spares: vec![Frame::new(area, None); count],
        }
    }

    #[must_use]
    pub(super) fn take(&mut self) -> Option<Frame> {
        self.spares.pop()
    }

    pub(super) fn place(&mut self, mut frame: Frame) {
        if frame.area() != self.area {
            frame.resize(self.area);
        }
        self.spares.push(frame);
    }

    pub(super) fn resize(&mut self, area: Area) {
        self.area = area;
        self.spares
            .iter_mut()
            .filter(|frame| frame.area() != area)
            .for_each(|frame| {
                frame.resize(area);
            });
    }
}
