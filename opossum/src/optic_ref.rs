#![warn(missing_docs)]
//! Module for storing references to optical nodes.
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    ser::SerializeStruct,
    Deserialize, Serialize,
};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::{
    analyzers::Analyzable,nodes::{create_node_ref, NodeAttr}, optic_senery_rsc::SceneryResources
};

#[derive(Debug, Clone)]
/// Structure for storing an optical node.
///
/// This structure stores a reference to an optical node (a structure implementing the
/// [`OpticNode`](crate::optic_node::OpticNode) trait). This [`OpticRef`] is then stored
/// as a node in a `NodeGroup`)[`crate::nodes::NodeGroup`].
pub struct OpticRef {
    /// The underlying optical reference.
    pub optical_ref: Arc<Mutex<dyn Analyzable>>,
}
impl OpticRef {
    /// Creates a new [`OpticRef`].
    pub fn new(
        node: Arc<Mutex<dyn Analyzable>>,
        global_conf: Option<Arc<Mutex<SceneryResources>>>,
    ) -> Self {
        node.lock().expect("Mutex lock failed").set_global_conf(global_conf);
        Self { optical_ref: node }
    }
    /// Returns the [`Uuid`] of the node, reference to by this [`OpticRef`].
    #[must_use]
    pub fn uuid(&self) -> Uuid {
        *self.optical_ref.lock().expect("Mutex lock failed").node_attr().uuid()
    }
    /// Update the reference to the global configuration.
    /// **Note**: This functions is normally only called from `OpticGraph`.
    pub fn update_global_config(&self, global_conf: Option<Arc<Mutex<SceneryResources>>>) {
        self.optical_ref.lock().expect("Mutex lock failed").set_global_conf(global_conf);
    }
}
impl Serialize for OpticRef {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut node = serializer.serialize_struct("node", 3)?;
        node.serialize_field("type", &self.optical_ref.lock().expect("Mutex lock failed").node_type())?;
        node.serialize_field("attributes", &self.optical_ref.lock().expect("Mutex lock failed").node_attr())?;
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
            Attributes,
        }
        const FIELDS: &[&str] = &["type", "attributes"];

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct FieldVisitor;

                impl Visitor<'_> for FieldVisitor {
                    type Value = Field;

                    fn expecting(
                        &self,
                        formatter: &mut std::fmt::Formatter<'_>,
                    ) -> std::fmt::Result {
                        formatter.write_str("`type`, or `attributes`")
                    }
                    fn visit_str<E>(self, value: &str) -> std::result::Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "type" => Ok(Field::NodeType),
                            "attributes" => Ok(Field::Attributes),
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
                let node =
                    create_node_ref(node_type).map_err(|e| de::Error::custom(e.to_string()))?;
                node.optical_ref
                    .lock().expect("Mutex lock failed")
                    .set_properties(properties)
                    .map_err(|e| de::Error::custom(e.to_string()))?;
                Ok(node)
            }

            fn visit_map<A>(self, mut map: A) -> std::result::Result<OpticRef, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut node_type = None;
                let mut node_attributes = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::NodeType => {
                            if node_type.is_some() {
                                return Err(de::Error::duplicate_field("type"));
                            }
                            node_type = Some(map.next_value()?);
                        }
                        Field::Attributes => {
                            if node_attributes.is_some() {
                                return Err(de::Error::duplicate_field("attributes"));
                            }
                            node_attributes = Some(map.next_value::<NodeAttr>()?);
                        }
                    }
                }
                let node_type = node_type.ok_or_else(|| de::Error::missing_field("type"))?;
                let node_attributes =
                    node_attributes.ok_or_else(|| de::Error::missing_field("attributes"))?;
                let node =
                    create_node_ref(node_type).map_err(|e| de::Error::custom(e.to_string()))?;
                node.optical_ref.lock().expect("Mutex lock failed").set_node_attr(node_attributes);
                // group node: assign props to graph
                node.optical_ref
                .lock().expect("Mutex lock failed")
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
    use crate::optic_node::OpticNode;
    use std::io::Read;
    use std::{fs::File, path::PathBuf};
    use uuid::uuid;
    #[test]
    fn new() {
        let uuid = Uuid::new_v4();
        let mut dummy = Dummy::default();
        dummy.node_attr_mut().set_uuid(&uuid);
        let optic_ref = OpticRef::new(Arc::new(Mutex::new(dummy)), None);
        assert_eq!(optic_ref.uuid(), uuid);
    }
    #[test]
    fn serialize() {
        let optic_ref = OpticRef::new(Arc::new(Mutex::new(Dummy::default())), None);
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
        let optic_ref = optic_ref.optical_ref.lock().expect("Mutex lock failed");
        assert_eq!(optic_ref.node_type(), "dummy");
        assert_eq!(optic_ref.name(), "test123");
    }
    #[test]
    fn debug() {
        assert_eq!(
            format!(
                "{:?}",
                OpticRef::new(Arc::new(Mutex::new(Dummy::default())), None)
            ),
            "OpticRef { optical_ref: RefCell { value: 'dummy' (dummy) } }"
        );
    }
}
