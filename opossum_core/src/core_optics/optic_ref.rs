#![warn(missing_docs)]
//! Module for storing references to optical nodes.
use serde::{
    Deserialize, Serialize,
    de::{self},
};
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::{
    analyzers::Analyzable,
    core_optics::{NodeAttr, NodeAttrExt, node_attr::HasNodeAttr},
    error::OpmResult,
    nodes::{NodeGroup, OpticGraph, create_node_ref},
    utils::LockExt,
};

#[derive(Clone)]
/// Structure for storing an optical node.
///
/// This structure stores a reference to an optical node (a structure implementing the
/// [`OpticNode`](crate::core_optics::OpticNode) trait). This [`OpticRef`] is then stored
/// as a node in a [`NodeGroup`].
pub struct OpticRef {
    /// The underlying optical reference.
    pub optical_ref: Arc<Mutex<dyn Analyzable>>,
}
impl OpticRef {
    /// Creates a new [`OpticRef`].
    ///
    /// # Panics
    ///
    /// This function might theoretically panic if locking of an internal mutex fails.
    pub fn new(node: Arc<Mutex<dyn Analyzable>>) -> Self {
        Self { optical_ref: node }
    }
    /// Returns the [`Uuid`] of the node, reference to by this [`OpticRef`].
    ///
    /// # Errors
    ///
    /// This function might theoretically return an error if the locking of an internal mutex fails.
    pub fn uuid(&self) -> OpmResult<Uuid> {
        Ok(self.optical_ref.lock_opm()?.node_attr().uuid())
    }
    /// Creates a deep copy of this optic reference with a fresh, independent node instance.
    ///
    /// # Errors
    ///
    /// This function might return an error if locking the internal optical reference was not
    /// successful.
    pub fn clone_deep(&self) -> OpmResult<Self> {
        let cloned_node = self.optical_ref.lock_opm()?.clone_analyzable();
        Ok(Self {
            optical_ref: cloned_node,
        })
    }
}
impl Debug for OpticRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpticRef")
            .field("optical_ref", &self.optical_ref.lock_opm().unwrap())
            .finish()
    }
}

// temporary helper struct which allows for attribute flattening
#[derive(Serialize)]
struct FlattenedOpticRefNodeAttr<'a> {
    #[serde(flatten)]
    attributes: &'a NodeAttr,
}

// temporary helper struct which allows for attribute flattening in a group node
#[derive(Serialize)]
struct FlattenedOpticRefGroup<'a> {
    #[serde(flatten)]
    attributes: &'a NodeAttr,
    graph: &'a OpticGraph,
}

impl Serialize for OpticRef {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let optical_ref = self.optical_ref.lock_opm().unwrap();

        if let Some(group_node) = optical_ref.as_any().downcast_ref::<NodeGroup>() {
            FlattenedOpticRefGroup {
                attributes: group_node.node_attr(),
                graph: group_node.graph(),
            }
            .serialize(serializer)
        } else {
            FlattenedOpticRefNodeAttr {
                attributes: optical_ref.node_attr(),
            }
            .serialize(serializer)
        }
    }
}
#[derive(Deserialize)]
struct OpticRefIntermediate {
    #[serde(flatten)]
    attributes: NodeAttr,
    #[serde(default)]
    graph: OpticGraph,
}

impl<'de> Deserialize<'de> for OpticRef {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut intermediate = OpticRefIntermediate::deserialize(deserializer)?;

        let node_type = intermediate.attributes.node_type();
        // Note: an unknown node type is not logged here. `OpticRef::deserialize` itself never skips a
        // node - callers that tolerate a failing node (currently `deserialize_nodes_lossy` in
        // `optic_graph/serialization.rs`) do the skipping and log accordingly, since only they know the
        // node was skipped rather than the whole document failing to load.
        let node_ref = create_node_ref(node_type).map_err(|e| de::Error::custom(e.to_string()))?;

        // Merge the deserialized properties on top of the node's default properties.
        // This ensures that:
        // 1. Properties not present in intermediate (e.g. skipped due to parse errors or omitted)
        //    remain at their default values.
        // 2. Property descriptions and validators registered on the default node are preserved.
        {
            let default_node = node_ref.optical_ref.lock_opm().unwrap();
            let mut merged_props = default_node.node_attr().properties().clone();
            drop(default_node);
            merged_props.update(intermediate.attributes.properties().clone());
            intermediate.attributes.set_properties(merged_props);
        }

        node_ref
            .optical_ref
            .lock_opm()
            .unwrap()
            .set_node_attr(intermediate.attributes)
            .map_err(|e| de::Error::custom(e.to_string()))?;

        // If the node is a group node, set its graph.
        if let Some(group_node) = node_ref
            .optical_ref
            .lock_opm()
            .unwrap()
            .as_any_mut()
            .downcast_mut::<NodeGroup>()
        {
            group_node.set_graph(intermediate.graph);
        }
        node_ref
            .optical_ref
            .lock_opm()
            .unwrap()
            .after_deserialization_hook()
            .map_err(|e| de::Error::custom(e.to_string()))?;

        Ok(node_ref)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        error::{OpmResult, OpossumError},
        nodes::Dummy,
        utils::LockExt,
    };
    use std::{fs::File, io::Read, path::PathBuf};
    use uuid::uuid;
    #[test]
    fn new() -> OpmResult<()> {
        let uuid = Uuid::new_v4();
        let mut dummy = Dummy::default();
        dummy.node_attr_mut().set_uuid(uuid);
        let optic_ref = OpticRef::new(Arc::new(Mutex::new(dummy)));
        assert_eq!(optic_ref.uuid()?, uuid);
        Ok(())
    }
    #[test]
    fn serialize() {
        let optic_ref = OpticRef::new(Arc::new(Mutex::new(Dummy::default())));
        assert!(
            ron::ser::to_string_pretty(&optic_ref, ron::ser::PrettyConfig::new().new_line("\n"))
                .is_ok()
        );
    }
    #[test]
    fn deserialize() -> OpmResult<()> {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("files_for_testing/opm/optic_ref.opm");
        let file_content = &mut "".to_owned();
        let _ = File::open(path)
            .map_err(|e| OpossumError::OpticScenery(format!("Error opening file: {e}")))?
            .read_to_string(file_content);
        let optic_ref: OpticRef = ron::from_str(&file_content).map_err(|e| {
            OpossumError::OpmDocument(format!("Error parsing opm file string: {e}"))
        })?;
        assert_eq!(
            optic_ref.uuid()?,
            uuid!("a2534789-ec98-4e9b-a1da-315a59d9da43")
        );
        let optic_ref = optic_ref.optical_ref.lock_opm()?;
        assert_eq!(optic_ref.node_type(), "dummy");
        assert_eq!(optic_ref.name(), "dummy1");
        Ok(())
    }
    #[test]
    fn debug() {
        assert_eq!(
            format!(
                "{:?}",
                OpticRef::new(Arc::new(Mutex::new(Dummy::default())))
            ),
            "OpticRef { optical_ref: 'dummy' (dummy) }"
        );
    }
}
