use std::{collections::BTreeMap, result, slice};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::{
    config::{Actions, Chord, Error, Result, notation},
    input::{
        action::{Action, OverlayKind, Scope},
        key::Key,
    },
};

mod trees;
pub use trees::{KeyTrees, Target};

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
///
/// `normal`, `select` and `insert` are the shared tables, which every text box answers to and
/// which hold only text box actions. A target adds a table of its own for each mode, which is
/// laid over the shared one when its keys are built.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "Merged")]
pub struct Keys {
    pub leader: Leader,
    pub normal: Bindings,
    pub select: Bindings,
    pub insert: Bindings,
    /// What the scratch binds besides the shared keys: its document and the editor.
    pub scratch: Overrides,
    /// What the test overlay binds, which is its own actions and the editor's.
    pub test: Overrides,
    /// What the command line binds, which is its own actions and the editor's.
    pub command: Overrides,
    /// What the search binds, which is its own actions and the editor's. Both of its boxes
    /// answer to it.
    pub search: Overrides,
    /// What the prompt that searches a text binds, which is its own actions and the editor's.
    pub find: Overrides,
}

/// What a target binds over the shared keys, for each mode. A combo bound to no actions is one
/// the target unbinds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overrides {
    pub normal: Bindings,
    pub select: Bindings,
    pub insert: Bindings,
}

/// The keys after a file went over the defaults, with the unbound combos still in.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Merged {
    leader: Leader,
    normal: Bindings,
    select: Bindings,
    insert: Bindings,
    scratch: ModeNames,
    test: ModeNames,
    command: ModeNames,
    search: ModeNames,
    find: ModeNames,
}

/// What a file binds in a target's table of each mode.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct ModeNames {
    normal: BTreeMap<Chord, Names>,
    select: BTreeMap<Chord, Names>,
    insert: BTreeMap<Chord, Names>,
}

/// What a file binds a combo of an overlay to: an action as text, or a list of them. They
/// are read once the table is known, because the table says which groups they may name.
#[derive(Deserialize)]
#[serde(untagged)]
enum Names {
    One(Box<str>),
    Many(Box<[Box<str>]>),
}

impl Names {
    fn actions(&self, scope: Scope) -> Result<Actions> {
        let names = match self {
            Self::One(name) => slice::from_ref(name),
            Self::Many(names) => names,
        };
        names
            .iter()
            .map(|name| Ok(Action::parse(scope, name)?))
            .collect::<Result<_>>()
            .map(Actions)
    }
}

/// Reads the actions of a target's tables in `scope`. The combos a file unbinds stay, as empty
/// lists, since they remove a shared combo when the keys are built.
fn overrides(scope: Scope, tables: &ModeNames) -> Result<Overrides> {
    let read = |table: &BTreeMap<Chord, Names>| -> Result<Bindings> {
        table
            .iter()
            .map(|(chord, names)| Ok((chord.clone(), names.actions(scope)?)))
            .collect()
    };
    Ok(Overrides {
        normal: read(&tables.normal)?,
        select: read(&tables.select)?,
        insert: read(&tables.insert)?,
    })
}

/// Checks that the shared tables hold only text box actions, so every text box can run what it
/// is bound to, and nothing global sits in the keys of a one-line box.
fn check_shared(mode: &'static str, bindings: &Bindings) -> Result<()> {
    for (chord, actions) in bindings {
        if actions
            .0
            .iter()
            .any(|action| !matches!(action, Action::TextBox(_)))
        {
            return Err(Error::NotTextBoxAction {
                mode,
                chord: chord.to_string().into(),
            });
        }
    }
    Ok(())
}

impl TryFrom<Merged> for Keys {
    type Error = Error;

    fn try_from(mut merged: Merged) -> Result<Self> {
        // A combo bound to no actions is how a file removes a default one
        for bindings in [&mut merged.normal, &mut merged.select, &mut merged.insert] {
            bindings.retain(|_, actions| !actions.0.is_empty());
        }
        check_shared("normal", &merged.normal)?;
        check_shared("select", &merged.select)?;
        check_shared("insert", &merged.insert)?;
        let keys = Self {
            leader: merged.leader,
            normal: merged.normal,
            select: merged.select,
            insert: merged.insert,
            scratch: overrides(Scope::Scratch, &merged.scratch)?,
            test: overrides(Scope::Overlay(OverlayKind::Test), &merged.test)?,
            command: overrides(Scope::Overlay(OverlayKind::Command), &merged.command)?,
            search: overrides(Scope::Overlay(OverlayKind::Search), &merged.search)?,
            find: overrides(Scope::Overlay(OverlayKind::Find), &merged.find)?,
        };
        // A file that collides is refused here, while it can still be reported
        keys.trees(Target::TextBox)?;
        keys.trees(Target::Scratch)?;
        for (name, kind) in [
            ("test", OverlayKind::Test),
            ("command", OverlayKind::Command),
            ("search", OverlayKind::Search),
            ("find", OverlayKind::Find),
        ] {
            keys.check_way_out(name, kind)?;
        }
        Ok(keys)
    }
}
