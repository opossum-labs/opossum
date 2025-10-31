#![warn(missing_docs)]
//! Module for storing references to optical nodes.
use serde::{
    Deserialize, Serialize,
    de::{self},
    ser::SerializeStruct,
};
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::{
    analyzers::Analyzable,
    nodes::{NodeAttr, OpticGraph, create_node_ref},
    optic_node::OpticNode,
    optic_scenery_rsc::SceneryResources,
    utils::LockExt,
};

#[derive(Clone)]
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
impl Serialize for OpticRef {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut optical_ref = self.optical_ref.lock_opm().unwrap();

        // We check if the node can be treated as a group node.
        // This avoids serializing the 'graph' field for non-group nodes.
        if let Ok(group_node) = optical_ref.as_group_mut() {
            let mut state = serializer.serialize_struct("OpticRef", 2)?;
            state.serialize_field("attributes", &group_node.node_attr())?;
            state.serialize_field("graph", &group_node.graph())?;
            state.end()
        } else {
            let mut state = serializer.serialize_struct("OpticRef", 1)?;
            state.serialize_field("attributes", &optical_ref.node_attr())?;
            drop(optical_ref);
            state.end()
        }
    }
}
#[derive(Deserialize)]
struct OpticRefIntermediate {
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
        let node_ref = create_node_ref(&node_type).map_err(|e| de::Error::custom(e.to_string()))?;
        node_ref
            .optical_ref
            .lock_opm()
            .unwrap()
            .set_node_attr(intermediate.attributes)
            .map_err(|e| de::Error::custom(e.to_string()))?;

        // If the node is a group node, set its graph.
        // The 'intermediate.graph' will always contain a valid OpticGraph
        // (either deserialized from the source or a default one).
        if let Ok(group_node) = node_ref.optical_ref.lock_opm().unwrap().as_group_mut() {
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
    use crate::nodes::Dummy;
    use crate::optic_node::OpticNode;
    use crate::utils::LockExt;
    use std::io::Read;
    use std::{fs::File, path::PathBuf};
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
        let _ =
            ron::ser::to_string_pretty(&optic_ref, ron::ser::PrettyConfig::new().new_line("\n"))
                .unwrap();
    }
    #[test]
    fn deserialize() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("files_for_testing/opm/optic_ref.opm");
        let file_content = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content);
        let optic_ref: OpticRef = ron::from_str(&file_content).unwrap();
        assert_eq!(
            optic_ref.uuid(),
            uuid!("98248e7f-dc4c-4131-8710-f3d5be2ff087")
        );
        let optic_ref = optic_ref.optical_ref.lock_opm().unwrap();
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
            "OpticRef { optical_ref: 'dummy' (dummy) }"
        );
    }
}
