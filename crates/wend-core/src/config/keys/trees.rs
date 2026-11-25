use super::{Bindings, Keys, Overrides};
use crate::{
    config::{Error, Result},
    input::{
        Mode,
        action::{Action, EditorAction, FindAction, OverlayAction, OverlayKind},
        key::{Key, KeyTree},
    },
};

/// Who a set of keys is built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A text box that binds nothing of its own.
    TextBox,
    Scratch,
    Overlay(OverlayKind),
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
    /// Builds the key tree of each mode for `target`, with `<leader>` resolved: the shared
    /// tables with the target's laid over them. Fails when a combo is bound as a key and also
    /// continued by a longer one.
    pub fn trees(&self, target: Target) -> Result<KeyTrees> {
        let leader = self.leader.0;
        let [normal, select, insert] = self.merged(target);
        Ok(KeyTrees {
            normal: build_tree("normal", &normal, leader)?,
            select: build_tree("select", &select, leader)?,
            insert: build_tree("insert", &insert, leader)?,
        })
    }

    /// Builds the key tree of the normal mode of `overlay`, which is all of the keys of an
    /// overlay that has no modes of its own.
    pub fn overlay_tree(&self, overlay: OverlayKind) -> Result<KeyTree> {
        Ok(self.trees(Target::Overlay(overlay))?.normal)
    }

    /// Returns the bindings of the normal, select and insert modes of `target`: the shared
    /// tables with the target's laid over them.
    fn merged(&self, target: Target) -> [Bindings; 3] {
        let overrides = self.overrides(target);
        [
            merged(&self.normal, overrides.map(|table| &table.normal)),
            merged(&self.select, overrides.map(|table| &table.select)),
            merged(&self.insert, overrides.map(|table| &table.insert)),
        ]
    }

    fn overrides(&self, target: Target) -> Option<&Overrides> {
        match target {
            Target::TextBox => None,
            Target::Scratch => Some(&self.scratch),
            Target::Overlay(OverlayKind::Test) => Some(&self.test),
            Target::Overlay(OverlayKind::Command) => Some(&self.command),
            Target::Overlay(OverlayKind::Search) => Some(&self.search),
            Target::Overlay(OverlayKind::Find) => Some(&self.find),
        }
    }
}

/// Lays `overrides` over `shared`: a combo of the override replaces the shared one, and one
/// bound to no actions removes it.
fn merged(shared: &Bindings, overrides: Option<&Bindings>) -> Bindings {
    let mut bindings = shared.clone();
    for (chord, actions) in overrides.into_iter().flatten() {
        if actions.0.is_empty() {
            bindings.remove(chord);
        } else {
            bindings.insert(chord.clone(), actions.clone());
        }
    }
    bindings
}

fn build_tree(mode: &'static str, bindings: &Bindings, leader: Key) -> Result<KeyTree> {
    let mut tree = KeyTree::new();
    for (chord, actions) in bindings {
        if !tree.insert(&chord.patterns(leader), actions.0.clone()) {
            return Err(Error::Conflict {
                mode,
                chord: chord.to_string().into(),
            });
        }
    }
    Ok(tree)
}

/// Whether `action` closes the overlay that has the keys, or the prompt of a text box.
fn closes_overlay(action: &Action) -> bool {
    matches!(
        action,
        Action::Editor(EditorAction::CloseOverlay)
            | Action::Overlay(OverlayAction::Find(FindAction::Close))
    )
}

impl Keys {
    /// Fails when nothing in the merged keys of `kind` closes it, since they would then trap the
    /// user.
    pub(super) fn check_way_out(&self, name: &'static str, kind: OverlayKind) -> Result<()> {
        // Also finds a collision in the merged keys
        self.trees(Target::Overlay(kind))?;
        let closes = self.merged(Target::Overlay(kind)).iter().any(|bindings| {
            bindings
                .values()
                .flat_map(|actions| &actions.0)
                .any(closes_overlay)
        });
        if closes {
            Ok(())
        } else {
            Err(Error::NoWayOut { overlay: name })
        }
    }
}
