use std::{collections::BTreeMap, result};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::{
    config::{Actions, Chord, Error, Result, notation},
    input::{
        Mode,
        key::{Key, KeyCode, KeyTree},
    },
};

mod default;

/// The combos of one mode and what each runs.
pub type Bindings = BTreeMap<Chord, Actions>;

/// The key that `<leader>` stands for in a chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leader(pub Key);

impl Serialize for Leader {
    fn serialize<S: Serializer>(&self, serializer: S) -> result::Result<S::Ok, S::Error> {
        let text = notation::format_key(self.0);
        // Always in brackets, so the leader reads the same whether it is a character or a name
        let bracketed = if text.starts_with('<') {
            text
        } else {
            format!("<{text}>")
        };
        serializer.serialize_str(&bracketed)
    }
}

impl<'de> Deserialize<'de> for Leader {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> result::Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let inner = text
            .strip_prefix('<')
            .and_then(|rest| rest.strip_suffix('>'))
            .unwrap_or(&text);
        if inner == "leader" {
            return Err(de::Error::custom(Error::LeaderIsLeader));
        }
        notation::parse_key(inner)
            .map(Self)
            .map_err(de::Error::custom)
    }
}

/// The keys of every mode. A file holds only what it changes: its bindings go over the
/// default ones, and an empty list of actions removes a default binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Overrides")]
pub struct Keys {
    pub leader: Leader,
    pub normal: Bindings,
    pub select: Bindings,
    pub insert: Bindings,
}

impl Default for Keys {
    fn default() -> Self {
        Self {
            leader: Leader(Key::new(KeyCode::Char(' '))),
            normal: default::command_bindings(Mode::Select),
            select: default::command_bindings(Mode::Normal),
            insert: default::insert_bindings(),
        }
    }
}

/// The key tree of each mode.
#[derive(Debug, Clone)]
pub struct KeyTrees {
    pub normal: KeyTree,
    pub select: KeyTree,
    pub insert: KeyTree,
}

impl KeyTrees {
    #[must_use]
    pub fn get(&self, mode: Mode) -> &KeyTree {
        match mode {
            Mode::Normal => &self.normal,
            Mode::Select => &self.select,
            Mode::Insert => &self.insert,
        }
    }
}

impl Keys {
    /// Builds the key tree of each mode, with `<leader>` resolved. Fails when a combo is
    /// bound as a key and also continued by a longer one.
    pub fn trees(&self) -> Result<KeyTrees> {
        let leader = self.leader.0;
        Ok(KeyTrees {
            normal: build_tree("normal", &self.normal, leader)?,
            select: build_tree("select", &self.select, leader)?,
            insert: build_tree("insert", &self.insert, leader)?,
        })
    }
}

fn build_tree(mode: &'static str, bindings: &Bindings, leader: Key) -> Result<KeyTree> {
    let mut tree = KeyTree::new();
    for (chord, actions) in bindings {
        if !tree.insert(&chord.keys(leader), actions.0.clone()) {
            return Err(Error::Conflict {
                mode,
                chord: chord.to_string().into(),
            });
        }
    }
    Ok(tree)
}

/// What a file says about the keys, before it goes over the defaults.
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct Overrides {
    leader: Option<Leader>,
    normal: Bindings,
    select: Bindings,
    insert: Bindings,
}

impl TryFrom<Overrides> for Keys {
    type Error = Error;

    fn try_from(overrides: Overrides) -> Result<Self> {
        let mut keys = Self::default();
        if let Some(leader) = overrides.leader {
            keys.leader = leader;
        }
        apply(&mut keys.normal, overrides.normal);
        apply(&mut keys.select, overrides.select);
        apply(&mut keys.insert, overrides.insert);
        // A file that collides is refused here, where its line can still be reported
        keys.trees()?;
        Ok(keys)
    }
}

fn apply(bindings: &mut Bindings, overrides: Bindings) {
    for (chord, actions) in overrides {
        if actions.0.is_empty() {
            bindings.remove(&chord);
        } else {
            bindings.insert(chord, actions);
        }
    }
}
