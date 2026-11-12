//! The config as a TOML document that explains itself, and the edits that keep a user's file
//! up to date with it.

use std::mem;

use anyhow::{Context, Result};
use toml_edit::{DocumentMut, Item, RawString, Table, de, ser};
use wend_core::config::{self, Config};

/// The default config, with a comment above every setting and next to every binding.
pub(super) fn default_document() -> Result<DocumentMut> {
    let config = Config::default();
    let mut document = ser::to_document(&config).context("failed to write the default config")?;
    expand_tables(document.as_table_mut());
    describe_settings(document.as_table_mut(), "");
    describe_bindings(&mut document, &config);
    Ok(document)
}

/// Turns the inline tables the serializer writes into sections, so each can carry a comment.
fn expand_tables(table: &mut Table) {
    for (_, item) in table.iter_mut() {
        *item = expanded(mem::take(item));
    }
}

fn expanded(item: Item) -> Item {
    match item.into_table() {
        Ok(mut table) => {
            expand_tables(&mut table);
            Item::Table(table)
        }
        Err(other) => other,
    }
}

/// Fills in what `existing` leaves out from the default config, and keeps everything the file
/// already says, comments included. Fails, with nothing to write, when the file or the result
/// isn't a valid config.
pub(super) fn extend(existing: &str) -> Result<String> {
    parse(existing).context("the config file is invalid")?;
    let mut document: DocumentMut = existing
        .parse()
        .context("the config file isn't valid TOML")?;
    merge(document.as_table_mut(), default_document()?.as_table());
    let text = document.to_string();
    parse(&text).context("the settings the file lacks collide with what it binds")?;
    Ok(text)
}

/// Reads a config from the text of a file.
pub(super) fn parse(text: &str) -> Result<Config> {
    Ok(de::from_str(text)?)
}

/// Adds what `defaults` has and `table` hasn't, going into the sections both have.
fn merge(table: &mut Table, defaults: &Table) {
    for (name, item) in defaults {
        match (table.get_mut(name), item) {
            (Some(Item::Table(inner)), Item::Table(default_inner)) => {
                describe_header(inner, default_inner);
                merge(inner, default_inner);
            }
            (Some(_), _) => {}
            (None, _) => {
                if let Some((key, _)) = defaults.get_key_value(name) {
                    table.insert_formatted(key, item.clone());
                }
            }
        }
    }
}

/// Gives a section of the file the comment the default one has, unless the file already put
/// something above it.
fn describe_header(table: &mut Table, defaults: &Table) {
    let existing = table.decor().prefix().and_then(RawString::as_str);
    if !existing.unwrap_or_default().trim().is_empty() {
        return;
    }
    if let Some(prefix) = defaults.decor().prefix().cloned() {
        table.decor_mut().set_prefix(prefix);
    }
}

/// Puts the comment of every setting above it. The bindings are described one by one in
/// `describe_bindings`, so what is under a mode isn't looked up here.
fn describe_settings(table: &mut Table, path: &str) {
    for (mut key, item) in table.iter_mut() {
        let path = if path.is_empty() {
            key.get().to_owned()
        } else {
            format!("{path}.{}", key.get())
        };
        let comment = config::describe(&path).map(comment_lines);
        match item {
            Item::Table(inner) => {
                if let Some(comment) = comment {
                    inner.decor_mut().set_prefix(format!("\n{comment}"));
                }
                if !matches!(path.as_str(), "keys.normal" | "keys.select" | "keys.insert") {
                    describe_settings(inner, &path);
                }
            }
            _ => {
                if let Some(comment) = comment {
                    key.leaf_decor_mut().set_prefix(comment);
                }
            }
        }
    }
}

/// Puts what each default binding does after it.
fn describe_bindings(document: &mut DocumentMut, config: &Config) {
    let modes = [
        ("normal", &config.keys.normal),
        ("select", &config.keys.select),
        ("insert", &config.keys.insert),
    ];
    for (mode, bindings) in modes {
        let Some(table) = document["keys"][mode].as_table_mut() else {
            continue;
        };
        for (chord, actions) in bindings {
            let Some(value) = table
                .get_mut(&chord.to_string())
                .and_then(Item::as_value_mut)
            else {
                continue;
            };
            let words: Vec<_> = actions
                .0
                .iter()
                .map(|action| action.description())
                .collect();
            value
                .decor_mut()
                .set_suffix(format!(" # {}", words.join(", then ")));
        }
    }
}

/// `text` as comment lines, each ended by a line break.
fn comment_lines(text: &str) -> String {
    let mut comment = String::new();
    for line in text.lines() {
        comment.push_str("# ");
        comment.push_str(line);
        comment.push('\n');
    }
    comment
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_text() -> String {
        default_document().unwrap().to_string()
    }

    #[test]
    fn the_default_document_reads_back_as_the_default_config() {
        assert_eq!(parse(&default_text()).unwrap(), Config::default());
    }

    /// The paths of every setting that has no comment of its own to look up: the values of the
    /// default document, and the sections of keys.
    fn paths(table: &Table, path: &str, found: &mut Vec<String>) {
        for (name, item) in table {
            let path = if path.is_empty() {
                name.to_owned()
            } else {
                format!("{path}.{name}")
            };
            match item {
                Item::Table(_) if path.starts_with("keys.") => found.push(path),
                Item::Table(inner) => paths(inner, &path, found),
                _ => found.push(path),
            }
        }
    }

    #[test]
    fn every_setting_is_described() {
        let document = default_document().unwrap();
        let mut found = vec!["keys".to_owned()];
        paths(document.as_table(), "", &mut found);
        assert!(found.len() > 5, "the walk found the settings");
        let text = document.to_string();
        for path in found {
            let comment =
                config::describe(&path).unwrap_or_else(|| panic!("{path} has no comment"));
            let first = comment.lines().next().unwrap();
            assert!(text.contains(&format!("# {first}")), "{path} is missing");
        }
    }

    #[test]
    fn every_default_binding_says_what_it_does() {
        let text = default_text();
        let bindings = text
            .lines()
            .skip_while(|line| *line != "[keys.normal]")
            .skip(1)
            .take_while(|line| !line.starts_with('['))
            .filter(|line| line.contains(" = "));
        for line in bindings {
            assert!(line.contains(" # "), "{line} has no comment");
        }
    }

    #[test]
    fn extending_an_empty_file_gives_the_default_document() {
        assert_eq!(extend("").unwrap(), default_text());
    }

    #[test]
    fn extending_keeps_what_the_file_says() {
        let existing = "# mine\n[scroll]\nmargin = 3 # tight\n\n[keys.normal]\nq = []\n";
        let text = extend(existing).unwrap();
        assert!(text.starts_with("# mine\n[scroll]\nmargin = 3 # tight\n"));
        let config = parse(&text).unwrap();
        assert_eq!(config.scroll.margin, 3);
        assert!(!config.keys.normal.contains_key(&"q".parse().unwrap()));
        assert_eq!(config.text, Config::default().text, "the rest is filled in");
        assert!(text.contains("[line_numbers]"));
        assert!(
            text.contains("# Keys. A combo"),
            "the comment of keys is added"
        );
        assert!(
            text.contains("# Keys in normal mode."),
            "and of a section the file had"
        );
    }

    #[test]
    fn extending_refuses_what_would_collide() {
        assert!(extend("[keys.normal]\ng = \"quit\"\n").is_err());
    }

    #[test]
    fn extending_refuses_an_invalid_file() {
        assert!(extend("[scroll]\nmargin = \"wide\"\n").is_err());
        assert!(extend("[scroll\n").is_err());
    }
}
