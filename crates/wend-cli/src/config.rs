//! Finds, reads and writes the config file, which the core never does itself.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use wend_core::config::Config;

mod document;

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

/// Reads the config file. No file means the default config, a file that doesn't parse is
/// an error.
pub(crate) fn load() -> Result<Config> {
    let Some(path) = config_path() else {
        return Ok(Config::default());
    };
    match read(&path)? {
        Some(text) => {
            document::parse(&text).with_context(|| format!("invalid config in {}", path.display()))
        }
        None => Ok(Config::default()),
    }
}

/// Fills in every setting the config file lacks, and creates the file if there is none.
/// Returns where it is.
pub(crate) fn extend() -> Result<PathBuf> {
    let path = writable_path()?;
    let existing = read(&path)?.unwrap_or_default();
    let text =
        document::extend(&existing).with_context(|| format!("can't extend {}", path.display()))?;
    write(&path, &text)?;
    Ok(path)
}

/// Replaces the config file with the default config, comments included. Returns where it is.
pub(crate) fn reset() -> Result<PathBuf> {
    let path = writable_path()?;
    write(&path, &document::default_document()?.to_string())?;
    Ok(path)
}

fn writable_path() -> Result<PathBuf> {
    config_path().ok_or_else(|| anyhow!("no config directory: set XDG_CONFIG_HOME or HOME"))
}

fn write(path: &Path, text: &str) -> Result<()> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)
            .with_context(|| format!("failed to create {}", directory.display()))?;
    }
    fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{document::parse, *};

    #[test]
    fn an_empty_file_is_the_default_config() {
        assert_eq!(parse("").unwrap(), Config::default());
    }

    #[test]
    fn a_binding_goes_over_the_defaults() {
        let config = parse("[keys.normal]\n\"<leader>e\" = \"save\"\n\"x\" = \"quit\"\n").unwrap();
        assert_eq!(
            config.keys.normal.len(),
            Config::default().keys.normal.len() + 2
        );
        let left = "h".parse().unwrap();
        assert_eq!(
            config.keys.normal[&left],
            Config::default().keys.normal[&left],
            "what the file doesn't mention stays"
        );
    }

    #[test]
    fn an_empty_list_unbinds() {
        let config = parse("[keys.normal]\nq = []\n").unwrap();
        assert!(!config.keys.normal.contains_key(&"q".parse().unwrap()));
    }

    #[test]
    fn the_leader_can_move() {
        let config = parse("[keys]\nleader = \"<C-a>\"\n").unwrap();
        assert_ne!(config.keys.leader, Config::default().keys.leader);
    }

    #[test]
    fn settings_are_read() {
        let config = parse("[text]\ntab_width = 2\n[scroll]\nmargin = 0\n").unwrap();
        assert_eq!(config.text.tab_width.get(), 2);
        assert_eq!(config.scroll.margin, 0);
    }

    #[test]
    fn bad_files_are_errors() {
        assert!(
            parse("[keys.normal]\nx = \"fly\"\n").is_err(),
            "no such action"
        );
        assert!(
            parse("[keys.normal]\n\"<nope>\" = \"quit\"\n").is_err(),
            "no such key"
        );
        assert!(
            parse("[keys.normal]\ng = \"quit\"\n").is_err(),
            "g starts gs and ge"
        );
        assert!(parse("[keys.norml]\n").is_err(), "a typo is not ignored");
        assert!(
            parse("[text]\ntab_width = 0\n").is_err(),
            "a tab has a width"
        );
    }
}
