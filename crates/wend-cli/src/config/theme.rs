//! Finds and reads theme files.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

use crate::theme::{self, Theme};

/// Where the theme files live, next to the config file.
fn directory() -> Option<PathBuf> {
    Some(super::config_path()?.with_file_name("themes"))
}

/// The name of a theme as it is written in the config, without a `.toml` ending a user may
/// have put on it.
fn plain_name(name: &str) -> &str {
    name.strip_suffix(".toml").unwrap_or(name)
}

/// The name becomes part of a path, so it is only letters, digits, `-` and `_`.
fn valid_name(name: &str) -> Result<()> {
    let plain = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if plain {
        Ok(())
    } else {
        bail!("`{name}` is not a theme name: use letters, digits, `-` and `_`")
    }
}

/// The theme called `name`: its file in the themes folder if there is one, else the built-in
/// theme of that name. Any other name is an error. Without a config directory only the
/// built-in themes exist.
pub(crate) fn load(name: &str) -> Result<Theme> {
    match directory() {
        Some(directory) => load_from(&directory, name),
        None => Theme::built_in(plain_name(name)).ok_or_else(|| no_theme(name)),
    }
}

fn load_from(directory: &Path, name: &str) -> Result<Theme> {
    let name = plain_name(name);
    valid_name(name)?;
    let path = directory.join(format!("{name}.toml"));
    match super::read(&path)? {
        Some(text) => text
            .parse()
            .with_context(|| format!("invalid theme in {}", path.display())),
        None => Theme::built_in(name).ok_or_else(|| no_theme(name)),
    }
}

/// The file of the built-in theme called `name`, to print for a user to copy.
pub(crate) fn built_in(name: &str) -> Result<&'static str> {
    theme::built_in_text(name).ok_or_else(|| no_theme(name))
}

fn no_theme(name: &str) -> anyhow::Error {
    let built_in: Vec<_> = theme::built_in_names().collect();
    anyhow!(
        "no theme `{name}`, the built-in themes are {}",
        built_in.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process};

    use crossterm::style::Color;

    use super::*;

    /// A themes folder of its own, removed again when the test is over.
    struct Folder(PathBuf);

    impl Folder {
        fn new(test: &str) -> Self {
            let folder = env::temp_dir().join(format!("wend-{test}-{}", process::id()));
            fs::create_dir_all(&folder).unwrap();
            Self(folder)
        }

        fn add(&self, name: &str, text: &str) {
            fs::write(self.0.join(format!("{name}.toml")), text).unwrap();
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    /// The default theme with its keyword colored `color`.
    fn recolored(color: &str) -> String {
        let default = theme::built_in_text("default").unwrap();
        let text = default.replace("#d6955b", color);
        assert_ne!(
            text, default,
            "the default's keyword color is where this expects it"
        );
        text
    }

    #[test]
    fn a_theme_name_is_not_a_path() {
        assert!(valid_name("dusk-2_b").is_ok(), "letters, digits, - and _");
        assert!(valid_name("../dusk").is_err(), "no way out of the folder");
        assert!(valid_name("a/b").is_err(), "no subfolders");
        assert!(valid_name("").is_err(), "a theme has a name");
    }

    #[test]
    fn every_built_in_theme_name_is_a_valid_name() {
        assert!(
            theme::built_in_names().count() > 1,
            "the themes folder is listed"
        );
        for name in theme::built_in_names() {
            assert!(
                valid_name(name).is_ok(),
                "{name} can be written in the config"
            );
        }
    }

    #[test]
    fn a_custom_theme_in_the_folder_is_loaded() {
        let folder = Folder::new("custom");
        folder.add("dusk", &recolored("#112233"));

        let theme = load_from(&folder.0, "dusk").unwrap();

        let blue = Color::Rgb {
            r: 0x11,
            g: 0x22,
            b: 0x33,
        };
        assert_eq!(theme.keyword.foreground, Some(blue), "the file's color");
        assert_eq!(
            theme.string,
            Theme::built_in("default").unwrap().string,
            "the rest is the file's too, which copied the default"
        );
    }

    #[test]
    fn a_toml_ending_on_the_name_is_dropped() {
        let folder = Folder::new("ending");
        folder.add("dusk", &recolored("#112233"));

        assert_eq!(
            load_from(&folder.0, "dusk.toml").unwrap(),
            load_from(&folder.0, "dusk").unwrap()
        );
    }

    #[test]
    fn a_file_goes_before_a_built_in_theme_of_the_same_name() {
        let folder = Folder::new("override");
        folder.add("nord", &recolored("#112233"));

        assert_ne!(
            load_from(&folder.0, "nord").unwrap(),
            Theme::built_in("nord").unwrap()
        );
    }

    #[test]
    fn a_built_in_theme_needs_no_file() {
        let folder = Folder::new("built-in");

        assert_eq!(
            load_from(&folder.0, "nord").unwrap(),
            Theme::built_in("nord").unwrap()
        );
    }

    #[test]
    fn a_theme_that_is_neither_is_an_error_naming_the_built_in_ones() {
        let folder = Folder::new("missing");

        let error = load_from(&folder.0, "dusk").unwrap_err().to_string();

        assert!(
            error.contains("no theme `dusk`") && error.contains("default"),
            "it names what was asked for and what there is"
        );
    }

    #[test]
    fn a_broken_theme_file_is_an_error_not_a_fallback() {
        let folder = Folder::new("broken");
        folder.add("nord", "[keyword]\nforeground = \"red\"\n");

        assert!(
            load_from(&folder.0, "nord").is_err(),
            "the built-in nord doesn't cover for the broken file"
        );
    }

    #[test]
    fn a_built_in_theme_is_printed_by_name() {
        assert!(built_in("default").is_ok(), "a built-in theme prints");
        assert!(
            built_in("nope")
                .unwrap_err()
                .to_string()
                .contains("default"),
            "an unknown name lists the built-in ones"
        );
    }
}
