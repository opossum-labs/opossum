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
    core_optics::{NodeAttr, NodeAttrExt, SceneryResources, node_attr::HasNodeAttr},
    nodes::{NodeGroup, OpticGraph, create_node_ref},
    utils::LockExt,
};

#[derive(Clone)]
/// Structure for storing an optical node.
///
/// This structure stores a reference to an optical node (a structure implementing the
/// [`OpticNode`] trait). This [`OpticRef`] is then stored
/// as a node in a `NodeGroup`)[`crate::nodes::NodeGroup`].
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
    pub fn new(
        node: Arc<Mutex<dyn Analyzable>>,
        global_conf: Option<Arc<Mutex<SceneryResources>>>,
    ) -> Self {
        node.lock_opm().unwrap().set_global_conf(global_conf);
        Self { optical_ref: node }
    }
    /// Returns the [`Uuid`] of the node, reference to by this [`OpticRef`].
    ///
    /// # Panics
    ///
    /// This function might theoretically panic if locking of an internal mutex fails.
    #[must_use]
    pub fn uuid(&self) -> Uuid {
        self.optical_ref.lock_opm().unwrap().node_attr().uuid()
    }
    /// Update the reference to the global configuration.
    /// **Note**: This functions is normally only called from `OpticGraph`.
    ///
    /// # Panics
    ///
    /// This function might theoretically panic if locking of an internal mutex fails.
    pub fn update_global_config(&self, global_conf: Option<Arc<Mutex<SceneryResources>>>) {
        self.optical_ref
            .lock_opm()
            .unwrap()
            .set_global_conf(global_conf);
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
        let intermediate = OpticRefIntermediate::deserialize(deserializer)?;

        let node_type = intermediate.attributes.node_type();
        let node_ref = create_node_ref(node_type).map_err(|e| de::Error::custom(e.to_string()))?;
        node_ref
            .optical_ref
            .lock_opm()
            .unwrap()
            .set_node_attr(intermediate.attributes)
            .map_err(|e| de::Error::custom(e.to_string()))?;

        // If the node is a group node, set its graph.
        // The 'intermediate.graph' will always contain a valid OpticGraph
        // (either deserialized from the source or a default one).
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
    fn new() {
        let uuid = Uuid::new_v4();
        let mut dummy = Dummy::default();
        dummy.node_attr_mut().set_uuid(uuid);
        let optic_ref = OpticRef::new(Arc::new(Mutex::new(dummy)), None);
        assert_eq!(optic_ref.uuid(), uuid);
    }
    #[test]
    fn serialize() {
        let optic_ref = OpticRef::new(Arc::new(Mutex::new(Dummy::default())), None);
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
            optic_ref.uuid(),
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
                OpticRef::new(Arc::new(Mutex::new(Dummy::default())), None)
            ),
            "OpticRef { optical_ref: 'dummy' (dummy) }"
        );
    }
}
