//! The core of the wend text editor: buffers, editing and state, with no
//! terminal attached. The terminal frontend is the `wend` crate, in
//! `crates/wend-cli`.

pub mod area;
pub mod buffer;
pub mod editor;
pub mod frame;
pub mod grapheme;
pub mod view;

/// The version of wend, as set in the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
