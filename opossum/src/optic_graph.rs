#![warn(missing_docs)]
use crate::{
    error::{OpmResult, OpossumError},
    light::Light,
    lightdata::LightData,
    nodes::NodeGroup,
    optic_ref::OpticRef,
    optic_senery_rsc::SceneryResources,
    optical::{LightResult, Optical},
    properties::Proptype,
    utils::geom_transformation::Isometry,
};
use log::warn;
use petgraph::{
    algo::{connected_components, is_cyclic_directed, toposort},
    graph::{EdgeIndex, Edges, Neighbors, NodeIndices},
    prelude::DiGraph,
    stable_graph::NodeIndex,
    visit::EdgeRef,
    Directed, Direction,
};
use serde::{
    de::{self, MapAccess, Visitor},
    ser::SerializeStruct,
    Deserialize, Serialize,
};
use std::{cell::RefCell, rc::Rc};
use uom::si::f64::Length;
use uuid::Uuid;

#[derive(Debug, Default, Clone)]
pub struct OpticGraph {
    g: DiGraph<OpticRef, Light>,
    global_confg: Option<Rc<RefCell<SceneryResources>>>,
}
impl OpticGraph {
    /// Add a new optical node to this [`OpticGraph`].
    ///
    /// This function returns a [`NodeIndex`] of the added node for later referencing (see `connect_nodes`).
    /// **Note**: While constructing the underlying [`OpticRef`] a rando, uuid is assigned.
    pub fn add_node<T: Optical + 'static>(&mut self, node: T) -> NodeIndex {
        self.g.add_node(OpticRef::new(
            Rc::new(RefCell::new(node)),
            None,
            self.global_confg.clone(),
        ))
    }
    /// Connect two optical nodes within this [`OpticGraph`].
    ///
    /// This function connects two optical nodes (referenced by their [`NodeIndex`]) with their respective port names and their geometrical distance
    /// (= propagation length) to each other thus extending the network.
    /// # Errors
    ///
    /// This function will return an error if
    ///   - the [`NodeIndex`] of source or target node does not exist in the [`OpticGraph`]
    ///   - a port name of the source or target node does not exist
    ///   - if a node/port combination was already connected earlier
    ///   - the connection of the nodes would form a loop in the network.
    ///   - the given geometric distance between the nodes is not finite.
    pub fn connect_nodes(
        &mut self,
        src_node: NodeIndex,
        src_port: &str,
        target_node: NodeIndex,
        target_port: &str,
        distance: Length,
    ) -> OpmResult<()> {
        let source = self.g.node_weight(src_node).ok_or_else(|| {
            OpossumError::OpticScenery("source node with given index does not exist".into())
        })?;
        if !source
            .optical_ref
            .borrow()
            .ports()
            .output_names()
            .contains(&src_port.into())
        {
            return Err(OpossumError::OpticScenery(format!(
                "source node {} does not have a port {}",
                source.optical_ref.borrow().name(),
                src_port
            )));
        }
        let target = self.g.node_weight(target_node).ok_or_else(|| {
            OpossumError::OpticScenery("target node with given index does not exist".into())
        })?;
        if !target
            .optical_ref
            .borrow()
            .ports()
            .input_names()
            .contains(&target_port.into())
        {
            return Err(OpossumError::OpticScenery(format!(
                "target node {} does not have a port {}",
                target.optical_ref.borrow().name(),
                target_port
            )));
        }

        if self.src_node_port_exists(src_node, src_port) {
            return Err(OpossumError::OpticScenery(format!(
                "src node <{}> with port <{}> is already connected",
                source.optical_ref.borrow().name(),
                src_port
            )));
        }
        if self.target_node_port_exists(target_node, target_port) {
            return Err(OpossumError::OpticScenery(format!(
                "target node <{}> with port <{}> is already connected",
                target.optical_ref.borrow().name(),
                target_port
            )));
        }
        let src_name = source.optical_ref.borrow().name();
        let target_name = target.optical_ref.borrow().name();
        let light = Light::new(src_port, target_port, distance)?;
        let edge_index = self.g.add_edge(src_node, target_node, light);
        if is_cyclic_directed(&self.g) {
            self.g.remove_edge(edge_index);
            return Err(OpossumError::OpticScenery(format!(
                "connecting nodes <{src_name}> -> <{target_name}> would form a loop"
            )));
        }
        Ok(())
    }
    /// Update reference to global config for each node in this [`OpticGraph`].
    /// This function is needed after deserialization.
    pub fn update_global_config(&mut self, global_conf: &Option<Rc<RefCell<SceneryResources>>>) {
        for node in self.g.node_weights_mut() {
            node.update_global_config(global_conf.clone());
        }
    }
    fn src_node_port_exists(&self, src_node: NodeIndex, src_port: &str) -> bool {
        self.g
            .edges_directed(src_node, petgraph::Direction::Outgoing)
            .any(|e| e.weight().src_port() == src_port)
    }
    fn target_node_port_exists(&self, target_node: NodeIndex, target_port: &str) -> bool {
        self.g
            .edges_directed(target_node, petgraph::Direction::Incoming)
            .any(|e| e.weight().target_port() == target_port)
    }
    fn node_by_uuid(&self, uuid: Uuid) -> Option<OpticRef> {
        self.g
            .node_weights()
            .find(|node| node.uuid() == uuid)
            .cloned()
    }
    /// Return a reference to the optical node specified by its node index.
    ///
    /// This function is mainly useful for setting up a reference node.
    ///
    /// # Errors
    ///
    /// This function will return [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node_by_idx(&self, node: NodeIndex) -> OpmResult<OpticRef> {
        let node = self
            .g
            .node_weight(node)
            .ok_or_else(|| OpossumError::OpticScenery("node index does not exist".into()))?;
        Ok(node.clone())
    }
    pub fn node_idxs(&self) -> NodeIndices {
        self.g.node_indices()
    }
    fn node_idx_by_uuid(&self, uuid: Uuid) -> Option<NodeIndex> {
        self.g
            .node_indices()
            .find(|idx| self.g.node_weight(*idx).unwrap().uuid() == uuid)
    }
    pub fn nodes(&self) -> Vec<&OpticRef> {
        self.g.node_weights().collect()
    }
    fn edge_by_idx(&self, idx: EdgeIndex) -> OpmResult<&Light> {
        self.g
            .edge_weight(idx)
            .ok_or_else(|| OpossumError::Other("could not get edge weight".into()))
    }
    pub fn topologically_sorted(&self) -> OpmResult<Vec<NodeIndex>> {
        toposort(&self.g, None)
            .map_err(|_| OpossumError::Analysis("topological sort failed".into()))
    }
    pub fn contains_detector(&self) -> bool {
        self.g
            .node_weights()
            .any(|node| node.optical_ref.borrow().is_detector())
    }
    pub fn is_single_tree(&self) -> bool {
        connected_components(&self.g) == 1
    }
    pub fn node_count(&self) -> usize {
        self.g.node_count()
    }
    pub fn edge_count(&self) -> usize {
        self.g.edge_count()
    }
    pub fn is_src_node(&self, idx: NodeIndex) -> bool {
        let group_srcs = self.g.externals(petgraph::Direction::Incoming);
        group_srcs.into_iter().any(|gs| gs == idx)
    }
    pub fn is_sink_node(&self, idx: NodeIndex) -> bool {
        let group_sinks = self.g.externals(petgraph::Direction::Outgoing);
        group_sinks.into_iter().any(|gs| gs == idx)
    }
    pub fn calc_node_isometry(&self, node_idx: NodeIndex) -> Option<Isometry> {
        let mut neighbors = self
            .g
            .neighbors_directed(node_idx, petgraph::Direction::Incoming);
        if let Some(neighbor) = neighbors.next() {
            let neighbor_node_ref = self.g.node_weight(neighbor).unwrap();
            let neighbor_node = neighbor_node_ref.optical_ref.borrow();
            let connecting_edge = self.g.edges_connecting(neighbor, node_idx).next().unwrap();
            let neighbor_output_port_name = connecting_edge.weight().src_port();
            if let Some(neighbor_iso) =
                neighbor_node.output_port_isometry(neighbor_output_port_name)
            {
                let distance = connecting_edge.weight().distance();
                let connecting_isometry = Isometry::new_along_z(*distance).unwrap();
                return Some(neighbor_iso.append(&connecting_isometry));
            }
        }
        None
    }
    pub fn incoming_edges(&self, idx: NodeIndex) -> LightResult {
        let edges = self.g.edges_directed(idx, petgraph::Direction::Incoming);
        edges
            .into_iter()
            .filter(|e| e.weight().data().is_some())
            .map(|e| {
                (
                    e.weight().target_port().to_owned(),
                    e.weight().data().cloned().unwrap(),
                )
            })
            .collect::<LightResult>()
    }
    pub fn set_outgoing_edge_data(&mut self, idx: NodeIndex, port: &str, data: LightData) {
        let edges = self.g.edges_directed(idx, Direction::Outgoing);
        let edge_ref = edges
            .into_iter()
            .filter(|idx| idx.weight().src_port() == port)
            .last();
        if let Some(edge_ref) = edge_ref {
            let edge_idx = edge_ref.id();
            let light = self.g.edge_weight_mut(edge_idx);
            if let Some(light) = light {
                light.set_data(Some(data));
            }
        } // else outgoing edge not connected -> data dropped
    }
    pub fn neighbors_undirected(&self, idx: NodeIndex) -> Neighbors<'_, Light> {
        self.g.neighbors_undirected(idx)
    }
    pub fn edges_directed(&self, idx: NodeIndex, dir: Direction) -> Edges<'_, Light, Directed> {
        self.g.edges_directed(idx, dir)
    }
    pub fn invert_graph(&mut self) -> OpmResult<()> {
        for node in self.g.node_weights_mut() {
            node.optical_ref
                .borrow_mut()
                .set_property("inverted", true.into())
                .map_err(|_| {
                    OpossumError::OpticGroup(
                        "group cannot be inverted because it contains a non-invertable node".into(),
                    )
                })?;
        }
        for edge in self.g.edge_weights_mut() {
            edge.inverse();
        }
        self.g.reverse();
        Ok(())
    }
    /// Creates the dot-format string which describes the edge that connects two nodes
    ///
    /// # Parameters:
    /// * `end_node_idx`:         [`NodeIndex`] of the node that should be connected
    /// * `light_port`:           port name that should be connected
    ///
    /// Returns the result of the edge strnig for the dot format
    fn create_node_edge_str(&self, end_node_idx: NodeIndex, light_port: &str) -> OpmResult<String> {
        let node_id = format!("i{}", self.node_by_idx(end_node_idx)?.uuid().as_simple());
        let node = self.node_by_idx(end_node_idx)?;
        if node.optical_ref.borrow().node_type() == "group" {
            let node = node.optical_ref.borrow();
            let group_node: &NodeGroup = node.as_group()?;
            Ok(group_node.get_mapped_port_str(light_port, &node_id)?)
        } else {
            Ok(format!("{node_id}:{light_port}"))
        }
    }
    pub fn create_dot_string(&self, rankdir: &str) -> OpmResult<String> {
        //check direction
        let rankdir = if rankdir == "LR" { "LR" } else { "TB" };
        let mut dot_string = String::default();
        for node in self.nodes() {
            let node_name = node.optical_ref.borrow().name();
            let inverted = node.optical_ref.borrow().properties().inverted()?;
            let ports = node.optical_ref.borrow().ports();
            dot_string += &node.optical_ref.borrow().to_dot(
                &format!("{}", node.uuid().as_simple()),
                &node_name,
                inverted,
                &ports,
                rankdir,
            )?;
        }
        for edge_idx in self.g.edge_indices() {
            let light: &Light = self.edge_by_idx(edge_idx)?;
            let end_nodes = self
                .g
                .edge_endpoints(edge_idx)
                .ok_or_else(|| OpossumError::Other("could not get edge_endpoints".into()))?;

            let src_edge_str = self.create_node_edge_str(end_nodes.0, light.src_port())?;
            let target_edge_str = self.create_node_edge_str(end_nodes.1, light.target_port())?;

            dot_string.push_str(&format!("  {src_edge_str} -> {target_edge_str} \n"));
        }
        dot_string.push_str("}\n");
        Ok(dot_string)
    }
}
impl Serialize for OpticGraph {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let g = self.g.clone();
        let mut graph = serializer.serialize_struct("graph", 2)?;
        let nodes = g.node_weights().cloned().collect::<Vec<OpticRef>>();
        graph.serialize_field("nodes", &nodes)?;
        let edgeidx = g
            .edge_indices()
            .map(|e| {
                (
                    g.node_weight(g.edge_endpoints(e).unwrap().0)
                        .unwrap()
                        .uuid(),
                    g.node_weight(g.edge_endpoints(e).unwrap().1)
                        .unwrap()
                        .uuid(),
                    g.edge_weight(e).unwrap().src_port(),
                    g.edge_weight(e).unwrap().target_port(),
                    *g.edge_weight(e).unwrap().distance(),
                )
            })
            .collect::<Vec<EdgeInfo<'_>>>();
        graph.serialize_field("edges", &edgeidx)?;
        graph.end()
    }
}

type EdgeInfo<'a> = (Uuid, Uuid, &'a str, &'a str, Length);

impl<'de> Deserialize<'de> for OpticGraph {
    #[allow(clippy::too_many_lines)]

    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        enum Field {
            Nodes,
            Edges,
        }
        const FIELDS: &[&str] = &["nodes", "edges"];

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
                        formatter.write_str("`nodes` or `edges`")
                    }
                    fn visit_str<E>(self, value: &str) -> std::result::Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "nodes" => Ok(Field::Nodes),
                            "edges" => Ok(Field::Edges),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct OpticGraphVisitor;

        impl<'de> Visitor<'de> for OpticGraphVisitor {
            type Value = OpticGraph;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an OpticGraph")
            }
            fn visit_map<A>(self, mut map: A) -> std::result::Result<OpticGraph, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut g = OpticGraph::default();
                let mut nodes: Option<Vec<OpticRef>> = None;
                let mut edges: Option<Vec<EdgeInfo<'_>>> = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Nodes => {
                            if nodes.is_some() {
                                return Err(de::Error::duplicate_field("nodes"));
                            }
                            nodes = Some(map.next_value::<Vec<OpticRef>>()?);
                        }
                        Field::Edges => {
                            if edges.is_some() {
                                return Err(de::Error::duplicate_field("edges"));
                            }
                            edges = Some(map.next_value::<Vec<EdgeInfo<'_>>>()?);
                        }
                    }
                }
                let nodes = nodes.ok_or_else(|| de::Error::missing_field("nodes"))?;
                let edges = edges.ok_or_else(|| de::Error::missing_field("edges"))?;
                for node in &nodes {
                    g.g.add_node(node.clone());
                }
                // assign references to ref nodes (if any)
                for node in &nodes {
                    if node.optical_ref.borrow().node_type() == "reference" {
                        let mut my_node = node.optical_ref.borrow_mut();
                        let refnode = my_node.as_refnode_mut().unwrap();
                        let node_props = refnode.properties().clone();
                        let uuid =
                            if let Proptype::Uuid(uuid) = node_props.get("reference id").unwrap() {
                                *uuid
                            } else {
                                Uuid::nil()
                            };
                        let Some(reference_node) = g.node_by_uuid(uuid) else {
                            return Err(de::Error::custom(
                                "reference node found, which does not reference anything",
                            ));
                        };
                        let ref_name =
                            format!("ref ({})", reference_node.optical_ref.borrow().name());
                        refnode.assign_reference(&reference_node);

                        refnode
                            .set_property("name", Proptype::String(ref_name))
                            .unwrap();
                    }
                }
                for edge in &edges {
                    let src_idx = g.node_idx_by_uuid(edge.0).ok_or_else(|| {
                        de::Error::custom(format!("src id {} does not exist", edge.0))
                    })?;
                    let target_idx = g.node_idx_by_uuid(edge.1).ok_or_else(|| {
                        de::Error::custom(format!("target id {} does not exist", edge.1))
                    })?;
                    g.connect_nodes(src_idx, edge.2, target_idx, edge.3, edge.4)
                        .map_err(|e| {
                            de::Error::custom(format!("connecting OpticGraph nodes failed: {e}"))
                        })?;
                }
                Ok(g)
            }
        }
        deserializer.deserialize_struct("OpticGraph", FIELDS, OpticGraphVisitor)
    }
}
impl From<OpticGraph> for Proptype {
    fn from(value: OpticGraph) -> Self {
        Self::OpticGraph(value)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{millimeter, nodes::Dummy};
    use num::Zero;

    #[test]
    fn add_node() {
        let mut graph = OpticGraph::default();
        graph.add_node(Dummy::new("n1"));
        assert_eq!(graph.g.node_count(), 1);
    }
    #[test]
    fn connect_nodes_ok() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::new("Test"));
        let n2 = graph.add_node(Dummy::new("Test"));
        assert!(graph
            .connect_nodes(n1, "rear", n2, "front", Length::zero())
            .is_ok());
        assert_eq!(graph.g.edge_count(), 1);
    }
    #[test]
    fn connect_nodes_failure() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::new("Test"));
        let n2 = graph.add_node(Dummy::new("Test"));
        assert!(graph
            .connect_nodes(n1, "rear", NodeIndex::new(5), "front", Length::zero())
            .is_err());
        assert!(graph
            .connect_nodes(NodeIndex::new(5), "rear", n2, "front", Length::zero())
            .is_err());
    }
    #[test]
    fn connect_nodes_wrong_distance() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::new("Test"));
        let n2 = graph.add_node(Dummy::new("Test"));
        assert!(graph
            .connect_nodes(n1, "rear", n2, "front", millimeter!(f64::NAN))
            .is_err());
        assert!(graph
            .connect_nodes(n1, "rear", n2, "front", millimeter!(f64::INFINITY))
            .is_err());
        assert!(graph
            .connect_nodes(n1, "rear", n2, "front", millimeter!(f64::NEG_INFINITY))
            .is_err());
    }
    #[test]
    fn connect_nodes_target_already_connected() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::new("Test"));
        let n2 = graph.add_node(Dummy::new("Test"));
        let n3 = graph.add_node(Dummy::new("Test"));
        assert!(graph
            .connect_nodes(n1, "rear", n2, "front", Length::zero())
            .is_ok());
        assert!(graph
            .connect_nodes(n3, "rear", n2, "front", Length::zero())
            .is_err());
        assert!(graph
            .connect_nodes(n1, "rear", n3, "front", Length::zero())
            .is_err());
    }
    #[test]
    fn connect_nodes_loop_error() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::new("Test"));
        let n2 = graph.add_node(Dummy::new("Test"));
        assert!(graph
            .connect_nodes(n1, "rear", n2, "front", Length::zero())
            .is_ok());
        assert!(graph
            .connect_nodes(n2, "rear", n1, "front", Length::zero())
            .is_err());
        assert_eq!(graph.g.edge_count(), 1);
    }
    #[test]
    fn node_by_uuid() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default());
        let uuid = graph.g.node_weight(n1).unwrap().uuid();
        assert!(graph.node_by_uuid(uuid).is_some());
        assert!(graph.node_by_uuid(Uuid::new_v4()).is_none());
    }
    #[test]
    fn node_id() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default());
        let uuid = graph.g.node_weight(n1).unwrap().uuid();
        assert_eq!(graph.node_idx_by_uuid(uuid), Some(n1));
        assert_eq!(graph.node_idx_by_uuid(Uuid::new_v4()), None);
    }
    #[test]
    fn is_single_tree() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default());
        let n2 = graph.add_node(Dummy::default());
        let n3 = graph.add_node(Dummy::default());
        let n4 = graph.add_node(Dummy::default());
        graph
            .connect_nodes(n1, "rear", n2, "front", Length::zero())
            .unwrap();
        graph
            .connect_nodes(n3, "rear", n4, "front", Length::zero())
            .unwrap();
        assert_eq!(graph.is_single_tree(), false);
        graph
            .connect_nodes(n2, "rear", n3, "front", Length::zero())
            .unwrap();
        assert_eq!(graph.is_single_tree(), true);
    }
    #[test]
    fn serialize_deserialize() {
        let mut graph = OpticGraph::default();
        graph.add_node(Dummy::default());
        let serialized = serde_yaml::to_string(&graph);
        assert!(serialized.is_ok());
        let serialized = serialized.unwrap();
        let deserialized: Result<OpticGraph, serde_yaml::Error> = serde_yaml::from_str(&serialized);
        assert!(deserialized.is_ok());
    }
}
