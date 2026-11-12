use std::fmt;

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, IntoDeserializer, SeqAccess, Visitor, value::SeqAccessDeserializer},
};

use crate::input::action::Action;

/// The actions one key combo runs, in order. A config writes a single one as a name and
/// several as a list of names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actions(pub Box<[Action]>);

impl Serialize for Actions {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &*self.0 {
            [action] => action.serialize(serializer),
            actions => actions.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Actions {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ActionsVisitor;

        impl<'de> Visitor<'de> for ActionsVisitor {
            type Value = Actions;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an action name, or a list of them")
            }

            fn visit_str<E: de::Error>(self, name: &str) -> Result<Actions, E> {
                let action = Action::deserialize(name.into_deserializer())?;
                Ok(Actions(Box::new([action])))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, sequence: A) -> Result<Actions, A::Error> {
                Box::deserialize(SeqAccessDeserializer::new(sequence)).map(Actions)
            }
        }

        deserializer.deserialize_any(ActionsVisitor)
    }
}
