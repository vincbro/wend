//! How keys are written in a config: a character stands for itself, and anything else goes
//! between angle brackets, with modifiers in front: `<C-x>`, `<A-S-tab>`, `<esc>`.

use crate::{
    config::{Error, Result},
    input::key::{Key, KeyCode, Modifiers},
};

const NAMED: [(&str, KeyCode); 7] = [
    ("esc", KeyCode::Esc),
    ("enter", KeyCode::Enter),
    ("tab", KeyCode::Tab),
    ("backspace", KeyCode::Backspace),
    ("space", KeyCode::Char(' ')),
    ("lt", KeyCode::Char('<')),
    ("gt", KeyCode::Char('>')),
];

/// Reads one key: a bare character, or the inside of a `<..>` without its brackets.
pub(super) fn parse_key(text: &str) -> Result<Key> {
    let mut chars = text.chars();
    if let (Some(character), None) = (chars.next(), chars.next()) {
        return Ok(Key::new(KeyCode::Char(character)));
    }

    let mut modifiers = Modifiers::default();
    let mut name = text;
    while let Some((flag, rest)) = take_modifier(name) {
        match flag {
            'C' => modifiers.control = true,
            'A' => modifiers.alt = true,
            _ => modifiers.shift = true,
        }
        name = rest;
    }

    let code = parse_code(name).ok_or_else(|| Error::UnknownKey(text.into()))?;
    if modifiers.shift && matches!(code, KeyCode::Char(_)) {
        return Err(Error::ShiftedCharacter(text.into()));
    }
    Ok(Key { code, modifiers })
}

/// Splits a leading `C-`, `A-` or `S-` off, unless that leaves nothing to name a key.
fn take_modifier(text: &str) -> Option<(char, &str)> {
    let mut chars = text.chars();
    let flag = chars
        .next()
        .filter(|flag| matches!(flag, 'C' | 'A' | 'S'))?;
    let rest = chars.as_str().strip_prefix('-')?;
    (!rest.is_empty()).then_some((flag, rest))
}

fn parse_code(name: &str) -> Option<KeyCode> {
    let mut chars = name.chars();
    if let (Some(character), None) = (chars.next(), chars.next()) {
        return Some(KeyCode::Char(character));
    }
    NAMED
        .iter()
        .find(|(known, _)| *known == name)
        .map(|&(_, code)| code)
}

/// Writes `key` so `parse_key` reads it back, in brackets when it isn't a bare character.
pub(super) fn format_key(key: Key) -> String {
    let Key { code, modifiers } = key;
    if let KeyCode::Char(character) = code
        && modifiers == Modifiers::default()
        && !matches!(character, '<' | ' ')
    {
        return character.to_string();
    }

    let mut text = String::from("<");
    for (is_set, flag) in [
        (modifiers.control, "C-"),
        (modifiers.alt, "A-"),
        (modifiers.shift, "S-"),
    ] {
        if is_set {
            text.push_str(flag);
        }
    }
    match NAMED.iter().find(|&&(_, named)| named == code) {
        Some((name, _)) => text.push_str(name),
        None => {
            if let KeyCode::Char(character) = code {
                text.push(character);
            }
        }
    }
    text.push('>');
    text
}
