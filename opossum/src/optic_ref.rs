use std::{cell::RefCell, rc::Rc};

use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    ser::SerializeStruct,
    Deserialize, Serialize,
};
use uuid::Uuid;

use crate::{nodes::create_node_ref, optical::Optical, properties::Properties};

#[derive(Debug, Clone)]
pub struct OpticRef {
    pub optical_ref: Rc<RefCell<dyn Optical>>,
    uuid: Uuid,
}

impl OpticRef {
    pub fn new(node: Rc<RefCell<dyn Optical>>, uuid: Option<Uuid>) -> Self {
        Self {
            optical_ref: node,
            uuid: uuid.unwrap_or_else(Uuid::new_v4),
        }
    }
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
        node.serialize_field(
            "type",
            self.optical_ref.borrow().properties().node_type().unwrap(),
        )?;
        node.serialize_field("id", &self.uuid)?;
        node.serialize_field("properties", &self.optical_ref.borrow().properties())?;
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
                    .borrow_mut()
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
                    .borrow_mut()
                    .set_properties(properties)
                    .map_err(|e| de::Error::custom(e.to_string()))?;
                // group node: assign props to graph
                node.optical_ref
                    .borrow_mut()
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
        let optic_ref = OpticRef::new(Rc::new(RefCell::new(Dummy::default())), Some(uuid));
        assert_eq!(optic_ref.uuid, uuid);
    }
    #[test]
    fn uuid() {
        let uuid = Uuid::new_v4();
        let optic_ref = OpticRef::new(Rc::new(RefCell::new(Dummy::default())), Some(uuid));
        assert_eq!(optic_ref.uuid(), uuid);
    }
    #[test]
    fn serialize() {
        let optic_ref = OpticRef::new(
            Rc::new(RefCell::new(Dummy::default())),
            Some(uuid!("587ee70f-6f52-4420-89f6-e1618ff4dbdb")),
        );
        let serialized = serde_json::to_string_pretty(&optic_ref);
        assert!(serialized.is_ok());
        // cannot check here easily because the order of properties is not defined...
        // let serialized=serialized.unwrap();
        // assert_eq!(&serialized,file_content);
    }
    #[test]
    fn deserialize() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("files_for_testing/opm/optic_ref.opm");
        let file_content = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content);
        let deserialized: Result<OpticRef, serde_json::Error> = serde_json::from_str(&file_content);
        assert!(deserialized.is_ok());
        let optic_ref = deserialized.unwrap();
        assert_eq!(
            optic_ref.uuid(),
            uuid!("587ee70f-6f52-4420-89f6-e1618ff4dbdb")
        );
        let optic_ref = optic_ref.optical_ref.borrow();
        let properties = optic_ref.properties();
        assert_eq!(properties.node_type().unwrap(), "dummy");
        assert_eq!(properties.name().unwrap(), "test123");
    }
    #[test]
    fn debug() {
        assert_eq!(
            format!(
                "{:?}",
                OpticRef::new(
                    Rc::new(RefCell::new(Dummy::default())),
                    Some(uuid!("587ee70f-6f52-4420-89f6-e1618ff4dbdb"))
                )
            ),
            "OpticRef { optical_ref: RefCell { value: dummy (dummy) }, uuid: 587ee70f-6f52-4420-89f6-e1618ff4dbdb }"
        );
    }
}
