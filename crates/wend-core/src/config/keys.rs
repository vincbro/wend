use std::{collections::BTreeMap, result};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::{
    config::{Actions, Chord, Error, Result, notation},
    input::{
        Mode,
        key::{Key, KeyTree},
    },
};

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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "Merged")]
pub struct Keys {
    pub leader: Leader,
    pub normal: Bindings,
    pub select: Bindings,
    pub insert: Bindings,
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

/// The keys after a file went over the defaults, with the unbound combos still in.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Merged {
    leader: Leader,
    normal: Bindings,
    select: Bindings,
    insert: Bindings,
}

impl TryFrom<Merged> for Keys {
    type Error = Error;

    fn try_from(mut merged: Merged) -> Result<Self> {
        // A combo bound to no actions is how a file removes a default one
        for bindings in [&mut merged.normal, &mut merged.select, &mut merged.insert] {
            bindings.retain(|_, actions| !actions.0.is_empty());
        }
        let keys = Self {
            leader: merged.leader,
            normal: merged.normal,
            select: merged.select,
            insert: merged.insert,
        };
        // A file that collides is refused here, while it can still be reported
        keys.trees()?;
        Ok(keys)
    }
}
