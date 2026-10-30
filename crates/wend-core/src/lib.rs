//! The core of the wend text editor: buffers, editing and state, with no
//! terminal attached. The terminal frontend is the `wend` crate, in
//! `crates/wend-cli`.

pub mod buffer;
pub mod editor;
pub mod frame;
pub mod view;
