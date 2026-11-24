use std::{fmt, result, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::{
    config::{Error, Result, notation},
    input::key::{Key, KeyCode, KeyPattern},
};

/// One step of a chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stroke {
    /// Whichever key the config makes the leader, so a whole family of combos moves together.
    Leader,
    /// Any key that types a character, written `<char>`.
    Character,
    Key(Key),
}

/// A key combo as a config writes it: `gs`, `<C-x>`, `<leader>e`. Parsing and printing it
/// are inverses, so a written config reads back the same.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Chord(Box<[Stroke]>);

impl Chord {
    /// The steps of the chord, with `leader` where it says `<leader>`.
    #[must_use]
    pub fn patterns(&self, leader: Key) -> Box<[KeyPattern]> {
        self.0
            .iter()
            .map(|stroke| match *stroke {
                Stroke::Leader => KeyPattern::Key(leader),
                Stroke::Character => KeyPattern::Character,
                Stroke::Key(key) => KeyPattern::Key(key),
            })
            .collect()
    }
}

impl FromStr for Chord {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        let mut strokes = Vec::new();
        let mut rest = text;
        while let Some(character) = rest.chars().next() {
            if character != '<' {
                strokes.push(Stroke::Key(Key::new(KeyCode::Char(character))));
                rest = &rest[character.len_utf8()..];
                continue;
            }
            let (inner, after) = rest[1..]
                .split_once('>')
                .ok_or_else(|| Error::UnclosedBracket(text.into()))?;
            strokes.push(match inner {
                "leader" => Stroke::Leader,
                "char" => Stroke::Character,
                _ => Stroke::Key(notation::parse_key(inner)?),
            });
            rest = after;
        }
        if strokes.is_empty() {
            return Err(Error::EmptyChord);
        }
        Ok(Self(strokes.into_boxed_slice()))
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for stroke in &self.0 {
            match *stroke {
                Stroke::Leader => formatter.write_str("<leader>")?,
                Stroke::Character => formatter.write_str("<char>")?,
                Stroke::Key(key) => formatter.write_str(&notation::format_key(key))?,
            }
        }
        Ok(())
    }
}

impl Serialize for Chord {
    fn serialize<S: Serializer>(&self, serializer: S) -> result::Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Chord {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> result::Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::key::Modifiers;

    fn chord(text: &str) -> Chord {
        text.parse().unwrap()
    }

    #[test]
    fn characters_are_strokes() {
        assert_eq!(
            chord("gs").0[..],
            [
                Stroke::Key(Key::new(KeyCode::Char('g'))),
                Stroke::Key(Key::new(KeyCode::Char('s')))
            ]
        );
    }

    #[test]
    fn brackets_hold_names_modifiers_and_the_leader() {
        let control_alt_x = Key {
            code: KeyCode::Char('x'),
            modifiers: Modifiers {
                control: true,
                alt: true,
                shift: false,
            },
        };
        assert_eq!(
            chord("<leader><C-A-x><esc>").0[..],
            [
                Stroke::Leader,
                Stroke::Key(control_alt_x),
                Stroke::Key(Key::new(KeyCode::Esc))
            ]
        );
    }

    #[test]
    fn leader_takes_the_place_of_its_stroke() {
        let space = Key::new(KeyCode::Char(' '));
        assert_eq!(
            chord("<leader>e").patterns(space)[..],
            [
                KeyPattern::Key(space),
                KeyPattern::Key(Key::new(KeyCode::Char('e')))
            ]
        );
    }

    #[test]
    fn printing_reads_back() {
        for text in [
            "gs",
            "<leader>e",
            "<char>",
            "<C-x>",
            "<A-S-tab>",
            "<space>",
            "<lt>",
            ">",
            "<C-gt>",
            "<C-->",
            "é",
        ] {
            assert_eq!(chord(text).to_string(), text);
        }
    }

    #[test]
    fn bad_chords_are_errors() {
        assert!(matches!("".parse::<Chord>(), Err(Error::EmptyChord)));
        assert!(matches!(
            "<esc".parse::<Chord>(),
            Err(Error::UnclosedBracket(_))
        ));
        assert!(matches!(
            "<nope>".parse::<Chord>(),
            Err(Error::UnknownKey(_))
        ));
        assert!(matches!(
            "<S-x>".parse::<Chord>(),
            Err(Error::ShiftedCharacter(_))
        ));
    }
}
