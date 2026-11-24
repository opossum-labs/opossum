#![warn(missing_docs)]
//! Module for storing references to optical nodes.
use nalgebra::Point2;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{
    Deserialize, Serialize, Serializer,
    de::{self},
};
use std::{borrow::Cow, fmt::Debug};
use uom::si::f64::Length;
use uuid::Uuid;

use crate::{
    analyzers::Analyzable, core_optics::{NodeAttr, NodeAttrExt, node_attr::NodePositioning}, nodes::{NodeGroup, OpticGraph, create_node_ref}, properties::Properties, types::{SiScalarSchema, schema_helper_types::Point2Schema}, utils::geom_transformation::Isometry,
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

impl JsonSchema for OpticRef {
    fn schema_name() -> Cow<'static, str> {
        DeserializedNode::schema_name()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        DeserializedNode::json_schema(generator)
    }
}

#[derive(Serialize)]
struct SerializedLeafNode<'a> {
    node_type: &'a str,
    name: &'a str,
    uuid: Uuid,
    #[serde(skip_serializing_if = "Properties::is_empty")]
    props: &'a Properties,
    #[serde(skip_serializing_if = "NodePositioning::is_automatic")]
    positioning: &'a NodePositioning,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    inverted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    alignment: &'a Option<Isometry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    align_like_node_at_distance: &'a Option<(Uuid, Length)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gui_position: &'a Option<Point2<f64>>,
}

#[derive(Serialize)]
struct SerializedGroupNode<'a> {
    node_type: &'a str,
    name: &'a str,
    uuid: Uuid,
    #[serde(skip_serializing_if = "Properties::is_empty")]
    props: &'a Properties,
    #[serde(skip_serializing_if = "NodePositioning::is_automatic")]
    positioning: &'a NodePositioning,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    inverted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    alignment: &'a Option<Isometry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    align_like_node_at_distance: &'a Option<(Uuid, Length)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gui_position: &'a Option<Point2<f64>>,
    graph: &'a OpticGraph,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "OpticNode", deny_unknown_fields)]
pub(crate) struct DeserializedNode {
    pub(crate) node_type: String,
    pub(crate) name: String,
    pub(crate) uuid: Uuid,
    #[serde(default)]
    pub(crate) props: Properties,
    #[serde(default)]
    pub(crate) positioning: NodePositioning,
    #[serde(default)]
    pub(crate) inverted: bool,
    #[serde(default)]
    pub(crate) alignment: Option<Isometry>,
    #[serde(default)]
    #[schemars(with = "Option<(Uuid, SiScalarSchema)>")]
    pub(crate) align_like_node_at_distance: Option<(Uuid, Length)>,
    #[serde(default)]
    #[schemars(with = "Option<Point2Schema>")]
    pub(crate) gui_position: Option<Point2<f64>>,
    /// Sub-graph contained inside this node (only populated when `node_type == "group"`).
    #[serde(default)]
    pub(crate) graph: OpticGraph,
}

impl DeserializedNode {
    /// Builds a `NodeAttr` by merging deserialized properties onto `default_props`.
    pub(crate) fn into_node_attr(self, mut default_props: Properties) -> (NodeAttr, OpticGraph) {
        default_props.update(&self.props);
        let mut attr = NodeAttr::new(&self.node_type);
        attr.set_name(&self.name);
        attr.set_uuid(self.uuid);
        attr.set_properties(default_props);
        attr.set_positioning(self.positioning);
        attr.set_inverted(self.inverted);
        attr.set_alignment_option(self.alignment);
        if let Some((target_id, dist)) = self.align_like_node_at_distance {
            attr.set_align_like_node_at_distance(target_id, dist);
        }
        attr.set_gui_position(self.gui_position);
        (attr, self.graph)
    }
}

pub(crate) fn serialize_node_flat<S>(
    attr: &NodeAttr,
    graph: Option<&OpticGraph>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if let Some(g) = graph.filter(|g| !g.is_empty()) {
        SerializedGroupNode {
            node_type: attr.node_type(),
            name: attr.name(),
            uuid: attr.uuid(),
            props: attr.properties(),
            positioning: attr.positioning(),
            inverted: attr.inverted(),
            alignment: attr.alignment(),
            align_like_node_at_distance: attr.get_align_like_node_at_distance(),
            gui_position: &attr.gui_position(),
            graph: g,
        }
        .serialize(serializer)
    } else {
        SerializedLeafNode {
            node_type: attr.node_type(),
            name: attr.name(),
            uuid: attr.uuid(),
            props: attr.properties(),
            positioning: attr.positioning(),
            inverted: attr.inverted(),
            alignment: attr.alignment(),
            align_like_node_at_distance: attr.get_align_like_node_at_distance(),
            gui_position: &attr.gui_position(),
        }
        .serialize(serializer)
    }
}

impl Serialize for OpticRef {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let graph = self
            .optical_ref
            .as_any()
            .downcast_ref::<NodeGroup>()
            .map(NodeGroup::graph);
        serialize_node_flat(self.optical_ref.node_attr(), graph, serializer)
    }
}

impl<'de> Deserialize<'de> for OpticRef {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = DeserializedNode::deserialize(deserializer)?;
        let mut node_ref =
            create_node_ref(&raw.node_type).map_err(|e| de::Error::custom(e.to_string()))?;

        let default_props = node_ref.node_attr().properties().clone();
        let (attr, graph) = raw.into_node_attr(default_props);

        node_ref
            .set_node_attr(attr)
            .map_err(|e| de::Error::custom(e.to_string()))?;

        if let Some(group_node) = node_ref.as_any_mut().downcast_mut::<NodeGroup>() {
            group_node.set_graph(graph);
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
        J_per_cm2,
        coatings::CoatingType,
        core_optics::{SurfaceConfig, node_attr::HasNodeAttr},
        error::{OpmResult, OpossumError},
        nodes::Dummy,
        properties::Proptype,
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
        let optic_ref: OpticRef = ron::from_str(file_content).map_err(|e| {
            OpossumError::OpmDocument(format!("Error parsing opm file string: {e}"))
        })?;
        assert_eq!(
            optic_ref.uuid(),
            uuid!("a2534789-ec98-4e9b-a1da-315a59d9da43")
        );
        assert_eq!(optic_ref.node_type(), "dummy");
        assert_eq!(optic_ref.name(), "dummy1");
        assert_eq!(
            optic_ref.node_attr().get_property("surface")?,
            &Proptype::SurfaceConfig(SurfaceConfig::new(CoatingType::IdealAR, J_per_cm2!(1.0))?)
        );
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
