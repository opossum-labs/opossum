#![warn(missing_docs)]
//! Module for storing references to optical nodes.
use serde::{
    Deserialize, Serialize,
    de::{self},
};
use std::fmt::Debug;
use uuid::Uuid;

use crate::{
    analyzers::Analyzable,
    core_optics::{NodeAttr, NodeAttrExt, node_attr::HasNodeAttr},
    nodes::{NodeGroup, OpticGraph, create_node_ref},
};

/// Structure for storing an optical node.
///
/// This structure stores an owned optical node (a structure implementing the
/// [`Analyzable`] trait). This [`OpticRef`] is stored as a node in an [`OpticGraph`].
pub struct OpticRef {
    /// The underlying optical node.
    pub optical_ref: Box<dyn Analyzable>,
}

impl OpticRef {
    /// Creates a new [`OpticRef`].
    #[must_use]
    pub fn new(node: Box<dyn Analyzable>) -> Self {
        Self { optical_ref: node }
    }
    /// Returns the [`Uuid`] of the node, reference to by this [`OpticRef`].
    #[must_use]
    pub fn uuid(&self) -> Uuid {
        self.optical_ref.node_attr().uuid()
    }
    /// Returns a reference to the inner [`Analyzable`].
    #[must_use]
    pub fn as_analyzable(&self) -> &dyn Analyzable {
        &*self.optical_ref
    }
    /// Returns a mutable reference to the inner [`Analyzable`].
    pub fn as_analyzable_mut(&mut self) -> &mut dyn Analyzable {
        &mut *self.optical_ref
    }
}

impl Clone for OpticRef {
    fn clone(&self) -> Self {
        Self {
            optical_ref: self.optical_ref.clone_analyzable(),
        }
    }
}

impl std::ops::Deref for OpticRef {
    type Target = dyn Analyzable;
    fn deref(&self) -> &Self::Target {
        &*self.optical_ref
    }
}

impl std::ops::DerefMut for OpticRef {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.optical_ref
    }
}

impl Debug for OpticRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpticRef")
            .field("optical_ref", &self.optical_ref)
            .finish()
    }
}

impl std::fmt::Display for OpticRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&**self, f)
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
        if let Some(group_node) = self.optical_ref.as_any().downcast_ref::<NodeGroup>() {
            FlattenedOpticRefGroup {
                attributes: group_node.node_attr(),
                graph: group_node.graph(),
            }
            .serialize(serializer)
        } else {
            FlattenedOpticRefNodeAttr {
                attributes: self.optical_ref.node_attr(),
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
        let mut node_ref =
            create_node_ref(node_type).map_err(|e| de::Error::custom(e.to_string()))?;

        // Merge the deserialized properties on top of the node's default properties.
        {
            let mut merged_props = node_ref.node_attr().properties().clone();
            merged_props.update(intermediate.attributes.properties().clone());
            intermediate.attributes.set_properties(merged_props);
        }

        node_ref
            .set_node_attr(intermediate.attributes)
            .map_err(|e| de::Error::custom(e.to_string()))?;

        // If the node is a group node, set its graph.
        if let Some(group_node) = node_ref.as_any_mut().downcast_mut::<NodeGroup>() {
            group_node.set_graph(intermediate.graph);
        }
        node_ref
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
    };
    use std::{fs::File, io::Read, path::PathBuf};
    use uuid::uuid;

    #[test]
    fn new() -> OpmResult<()> {
        let uuid = Uuid::new_v4();
        let mut dummy = Dummy::default();
        dummy.node_attr_mut().set_uuid(uuid);
        let optic_ref = OpticRef::new(Box::new(dummy));
        assert_eq!(optic_ref.uuid(), uuid);
        Ok(())
    }

    #[test]
    fn serialize() {
        let optic_ref = OpticRef::new(Box::new(Dummy::default()));
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
        assert_eq!(optic_ref.node_type(), "dummy");
        assert_eq!(optic_ref.name(), "dummy1");
        Ok(())
    }

    #[test]
    fn debug() {
        assert_eq!(
            format!("{:?}", OpticRef::new(Box::new(Dummy::default()))),
            "OpticRef { optical_ref: 'dummy' (dummy) }"
        );
    }
}
