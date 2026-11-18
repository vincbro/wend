//! What a user can change about the editor. The frontend reads it from wherever it lives and
//! sends it in, because the core never touches the disk. `config/default.toml` in the repo is
//! the one place the defaults are written down: `DEFAULT` is that file, and a user's file
//! goes over it.

use serde::Deserialize;
use toml_edit::{DocumentMut, Item, Table, de};

mod actions;
mod chord;
mod error;
mod keys;
mod notation;
mod settings;
pub use actions::Actions;
pub use chord::{Chord, Stroke};
pub use error::{Error, Result};
pub use keys::{Bindings, KeyTrees, Keys, Leader};
pub use settings::{LineNumbers, Scroll, Text};

/// The default config as a file, with a comment on every setting. A config file only needs to
/// hold what it changes.
pub const DEFAULT: &str = include_str!("../../../config/default.toml");

/// The settings of the editor.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The name of the theme the frontend draws with. The core only carries it.
    pub theme: Box<str>,
    pub keys: Keys,
    pub scroll: Scroll,
    pub text: Text,
    pub line_numbers: LineNumbers,
}

impl Default for Config {
    fn default() -> Self {
        Self::parse("").expect("the default config file is a valid config")
    }
}

impl Config {
    /// Reads the text of a config file. A setting it leaves out keeps its value from
    /// `DEFAULT`, and a combo it binds to an empty list loses its default binding.
    pub fn parse(text: &str) -> Result<Self> {
        let mut document: DocumentMut = DEFAULT.parse()?;
        let file: DocumentMut = text.parse()?;
        merge(document.as_table_mut(), file.into_table());
        Ok(de::from_document(document)?)
    }
}

/// Lays what `file` says over `base`, going into the sections both have.
fn merge(base: &mut Table, file: Table) {
    for (key, item) in file {
        // An inline table is a section too, so `scroll = { margin = 0 }` merges like `[scroll]`
        match item.into_table() {
            Ok(section) => merge_section(base, &key, section),
            Err(value) => {
                base.insert(&key, value);
            }
        }
    }
}

/// Merges `section` into the section `base` has under `key`, or adds it when `base` has none.
fn merge_section(base: &mut Table, key: &str, section: Table) {
    match base.get_mut(key) {
        Some(Item::Table(inner)) => merge(inner, section),
        _ => {
            base.insert(key, Item::Table(section));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(text: &str) -> Chord {
        text.parse().unwrap()
    }

    #[test]
    fn the_default_file_is_a_config() {
        let config = Config::default();

        assert_eq!(&*config.theme, "default");
        assert!(
            config.keys.normal.contains_key(&chord("h")),
            "it binds keys"
        );
        assert!(config.keys.insert.contains_key(&chord("<esc>")));
    }

    #[test]
    fn an_empty_file_is_the_default_config() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }

    #[test]
    fn a_binding_goes_over_the_defaults() {
        let config =
            Config::parse("[keys.normal]\n\"<leader>z\" = \"save\"\n\"z\" = \"quit\"\n").unwrap();
        let default = Config::default();

        assert_eq!(config.keys.normal.len(), default.keys.normal.len() + 2);
        assert_eq!(
            config.keys.normal[&chord("h")],
            default.keys.normal[&chord("h")],
            "what the file doesn't mention stays"
        );
    }

    #[test]
    fn an_empty_list_unbinds() {
        let config = Config::parse("[keys.normal]\nq = []\n").unwrap();

        assert!(!config.keys.normal.contains_key(&chord("q")));
    }

    #[test]
    fn the_leader_can_move() {
        let config = Config::parse("[keys]\nleader = \"<C-a>\"\n").unwrap();

        assert_ne!(config.keys.leader, Config::default().keys.leader);
    }

    #[test]
    fn settings_are_read_over_the_defaults() {
        let config = Config::parse("theme = \"dusk\"\n[text]\ntab_width = 2\n").unwrap();

        assert_eq!(&*config.theme, "dusk");
        assert_eq!(config.text.tab_width.get(), 2);
        assert_eq!(
            config.scroll,
            Config::default().scroll,
            "a section the file lacks stays"
        );
    }

    #[test]
    fn an_inline_table_merges_like_a_section() {
        let config = Config::parse("scroll = { margin = 0 }\n").unwrap();

        assert_eq!(config.scroll.margin, 0);
        assert_eq!(config.keys, Config::default().keys);
    }

    #[test]
    fn bad_files_are_errors() {
        assert!(
            Config::parse("[keys.normal]\nx = \"fly\"\n").is_err(),
            "no such action"
        );
        assert!(
            Config::parse("[keys.normal]\n\"<nope>\" = \"quit\"\n").is_err(),
            "no such key"
        );
        assert!(
            Config::parse("[keys.normal]\ng = \"quit\"\n").is_err(),
            "g starts gs and ge"
        );
        assert!(
            Config::parse("[keys.norml]\n").is_err(),
            "a typo is not ignored"
        );
        assert!(
            Config::parse("[text]\ntab_width = 0\n").is_err(),
            "a tab has a width"
        );
        assert!(Config::parse("[scroll\n").is_err(), "not TOML");
    }
}
