use super::{Canvas, Layout};
use crate::frame::Rect;

/// Something that draws into the frame, and where it wants to be to do it.
///
/// An element keeps its own children, if it has any, and draws each with `Canvas::child`.
pub trait Element {
    fn layout(&self) -> Layout;

    /// Draws into `rect`, which `layout` already fitted inside the parent's space.
    fn render(&mut self, rect: Rect, canvas: &mut Canvas<'_>);
}
