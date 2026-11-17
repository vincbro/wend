//! Finds and reads the config file and the theme files, which the core never does itself.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use wend_core::config::Config;
pub(crate) use wend_core::config::DEFAULT;

pub(crate) mod theme;

/// Where the config lives: `$XDG_CONFIG_HOME/wend/config.toml`, or under `~/.config`.
fn config_path() -> Option<PathBuf> {
    let directory = env::var_os("XDG_CONFIG_HOME")
        .filter(|directory| !directory.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| Path::new(&home).join(".config")))?;
    Some(directory.join("wend").join("config.toml"))
}

/// The text of the config file, none when there is no file.
fn read(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

/// Reads the config file. With no file the default config is used, and a file that doesn't
/// parse is an error.
pub(crate) fn load() -> Result<Config> {
    match config_path() {
        Some(path) => load_from(&path),
        None => Ok(Config::default()),
    }
}

fn load_from(path: &Path) -> Result<Config> {
    let Some(text) = read(path)? else {
        return Ok(Config::default());
    };
    Config::parse(&text).with_context(|| format!("invalid config in {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::process;

    use super::*;

    #[test]
    fn no_file_is_the_default_config_and_writes_nothing() {
        let path = env::temp_dir()
            .join("wend-no-such-directory")
            .join("config.toml");

        assert_eq!(load_from(&path).unwrap(), Config::default());
        assert!(!path.exists(), "reading never creates the file");
    }

    #[test]
    fn a_file_that_doesnt_parse_is_an_error() {
        let path = env::temp_dir().join(format!("wend-bad-config-{}.toml", process::id()));
        fs::write(&path, "[scroll]\nmargin = \"wide\"\n").unwrap();

        let result = load_from(&path);
        fs::remove_file(&path).unwrap();

        assert!(
            result.is_err(),
            "a mistake in the file is shown, not ignored"
        );
    }
}
