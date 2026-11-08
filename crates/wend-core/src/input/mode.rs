/// Decides how keys are read: as commands, as commands that extend the selection, or as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    /// Works like normal, except motions extend the selection instead of moving it.
    Select,
    Insert,
}
