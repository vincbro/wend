//! What a user can change about the editor. The frontend reads it from wherever it lives and
//! sends it in, because the core never touches the disk. `Config::default` is the config
//! nobody wrote, and writing it out gives the user a file to start from.

use serde::{Deserialize, Serialize};

mod actions;
mod chord;
mod docs;
mod error;
mod keys;
mod notation;
mod settings;
pub use actions::Actions;
pub use chord::{Chord, Stroke};
pub use docs::describe;
pub use error::{Error, Result};
pub use keys::{Bindings, KeyTrees, Keys, Leader};
pub use settings::{LineNumbers, Scroll, Text};

/// The settings of the editor. Whatever a file leaves out keeps its default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub keys: Keys,
    pub scroll: Scroll,
    pub text: Text,
    pub line_numbers: LineNumbers,
}
