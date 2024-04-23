#![warn(missing_docs)]
//! Module for storing references to optical nodes.
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    ser::SerializeStruct,
    Deserialize, Serialize,
};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::{nodes::create_node_ref, optical::Optical, properties::Properties};

#[derive(Debug, Clone)]
/// Structure for storing an optical node.
///
/// This structure stores a reference to an optical node (a structure implementing the [`Optical`] trait). This [`OpticRef`]
/// is then stored as a node in an `OpticGraph` (i.e. (`OpticScenery`)[`crate::OpticScenery`] or (`NodeGroup`)[`crate::nodes::NodeGroup`]).
/// In addition, it contains a unique id ([`Uuid`]) in order to unambiguously identify a node within a scene.
pub struct OpticRef {
    /// The underlying optical reference.
    pub optical_ref: Arc<Mutex<dyn Optical>>,
    uuid: Uuid,
}

impl OpticRef {
    /// Creates a new [`OpticRef`].
    ///
    /// You can either assign a given [`Uuid`] using `Some(uuid!(...))` or provide `None`, where a new unique id is generated.
    pub fn new(node: Arc<Mutex<dyn Optical>>, uuid: Option<Uuid>) -> Self {
        Self {
            optical_ref: node,
            uuid: uuid.unwrap_or_else(Uuid::new_v4),
        }
    }
    /// Returns the [`Uuid`] of this [`OpticRef`].
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.uuid
    }
}
impl Serialize for OpticRef {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut node = serializer.serialize_struct("node", 3)?;
        node.serialize_field("type", &self.optical_ref.lock().unwrap().node_type())?;
        node.serialize_field("id", &self.uuid)?;
        node.serialize_field("properties", &self.optical_ref.lock().unwrap().properties())?;
        node.end()
    }
}

impl<'de> Deserialize<'de> for OpticRef {
    #[allow(clippy::too_many_lines)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        enum Field {
            NodeType,
            Properties,
            Id,
        }
        const FIELDS: &[&str] = &["type", "properties", "id"];

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(
                        &self,
                        formatter: &mut std::fmt::Formatter<'_>,
                    ) -> std::fmt::Result {
                        formatter.write_str("`type`, `properties`, or `id`")
                    }
                    fn visit_str<E>(self, value: &str) -> std::result::Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "type" => Ok(Field::NodeType),
                            "properties" => Ok(Field::Properties),
                            "id" => Ok(Field::Id),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct OpticRefVisitor;

        impl<'de> Visitor<'de> for OpticRefVisitor {
            type Value = OpticRef;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a struct OpticRef")
            }

            fn visit_seq<A>(self, mut seq: A) -> std::result::Result<OpticRef, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let node_type = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let properties = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let node = create_node_ref(node_type, None)
                    .map_err(|e| de::Error::custom(e.to_string()))?;
                node.optical_ref
                    .lock().unwrap()
                    .set_properties(properties)
                    .map_err(|e| de::Error::custom(e.to_string()))?;
                Ok(node)
            }

            fn visit_map<A>(self, mut map: A) -> std::result::Result<OpticRef, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut node_type = None;
                let mut properties = None;
                let mut id: Option<Uuid> = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::NodeType => {
                            if node_type.is_some() {
                                return Err(de::Error::duplicate_field("type"));
                            }
                            node_type = Some(map.next_value()?);
                        }
                        Field::Properties => {
                            if properties.is_some() {
                                return Err(de::Error::duplicate_field("properties"));
                            }
                            properties = Some(map.next_value::<Properties>()?);
                        }
                        Field::Id => {
                            if id.is_some() {
                                return Err(de::Error::duplicate_field("id"));
                            }
                            id = Some(map.next_value()?);
                        }
                    }
                }
                let node_type = node_type.ok_or_else(|| de::Error::missing_field("type"))?;
                let properties =
                    properties.ok_or_else(|| de::Error::missing_field("properties"))?;
                let id = id.ok_or_else(|| de::Error::missing_field("id"))?;
                let node = create_node_ref(node_type, Some(id))
                    .map_err(|e| de::Error::custom(e.to_string()))?;
                node.optical_ref
                    .lock().unwrap()
                    .set_properties(properties)
                    .map_err(|e| de::Error::custom(e.to_string()))?;
                // group node: assign props to graph
                node.optical_ref
                    .lock().unwrap()
                    .after_deserialization_hook()
                    .map_err(|e| de::Error::custom(e.to_string()))?;
                Ok(node)
            }
        }
        deserializer.deserialize_struct("OpticRef", FIELDS, OpticRefVisitor)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::nodes::Dummy;
    use std::io::Read;
    use std::{fs::File, path::PathBuf};
    use uuid::uuid;
    #[test]
    fn new() {
        let uuid = Uuid::new_v4();
        let optic_ref = OpticRef::new(Arc::new(Mutex::new(Dummy::default())), Some(uuid));
        assert_eq!(optic_ref.uuid, uuid);
    }
    #[test]
    fn uuid() {
        let uuid = Uuid::new_v4();
        let optic_ref = OpticRef::new(Arc::new(Mutex::new(Dummy::default())), Some(uuid));
        assert_eq!(optic_ref.uuid(), uuid);
    }
    #[test]
    fn serialize() {
        let optic_ref = OpticRef::new(
            Arc::new(Mutex::new(Dummy::default())),
            Some(uuid!("587ee70f-6f52-4420-89f6-e1618ff4dbdb")),
        );
        let serialized = serde_yaml::to_string(&optic_ref);
        assert!(serialized.is_ok());
    }
    #[test]
    fn deserialize() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("files_for_testing/opm/optic_ref.opm");
        let file_content = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content);
        let deserialized: Result<OpticRef, serde_yaml::Error> = serde_yaml::from_str(&file_content);
        assert!(deserialized.is_ok());
        let optic_ref = deserialized.unwrap();
        assert_eq!(
            optic_ref.uuid(),
            uuid!("587ee70f-6f52-4420-89f6-e1618ff4dbdb")
        );
        let optic_ref = optic_ref.optical_ref.lock().unwrap();
        assert_eq!(optic_ref.node_type(), "dummy");
        assert_eq!(optic_ref.name(), "test123");
    }
    #[test]
    fn debug() {
        assert_eq!(
            format!(
                "{:?}",
                OpticRef::new(
                    Arc::new(Mutex::new(Dummy::default())),
                    Some(uuid!("587ee70f-6f52-4420-89f6-e1618ff4dbdb"))
                )
            ),
            "OpticRef { optical_ref: Mutex { value: dummy (dummy) }, uuid: 587ee70f-6f52-4420-89f6-e1618ff4dbdb }"
        );
    }
}
