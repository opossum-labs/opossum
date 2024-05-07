#![warn(missing_docs)]
use crate::{
    error::{OpmResult, OpossumError},
    light::Light,
    optic_ref::OpticRef,
    optical::Optical,
    properties::Proptype,
    utils::geom_transformation::Isometry,
};
use log::warn;
use petgraph::{
    algo::{connected_components, is_cyclic_directed},
    prelude::DiGraph,
    stable_graph::NodeIndex,
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
pub struct OpticGraph(pub DiGraph<OpticRef, Light>);

impl OpticGraph {
    pub fn add_node<T: Optical + 'static>(&mut self, node: T) -> NodeIndex {
        self.0
            .add_node(OpticRef::new(Rc::new(RefCell::new(node)), None))
    }
    pub fn connect_nodes(
        &mut self,
        src_node: NodeIndex,
        src_port: &str,
        target_node: NodeIndex,
        target_port: &str,
        distance: Length,
    ) -> OpmResult<()> {
        let source = self.0.node_weight(src_node).ok_or_else(|| {
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
        let target = self.0.node_weight(target_node).ok_or_else(|| {
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
        let edge_index = self.0.add_edge(src_node, target_node, light);
        if is_cyclic_directed(&self.0) {
            self.0.remove_edge(edge_index);
            return Err(OpossumError::OpticScenery(format!(
                "connecting nodes <{src_name}> -> <{target_name}> would form a loop"
            )));
        }
        Ok(())
    }
    fn src_node_port_exists(&self, src_node: NodeIndex, src_port: &str) -> bool {
        self.0
            .edges_directed(src_node, petgraph::Direction::Outgoing)
            .any(|e| e.weight().src_port() == src_port)
    }
    fn target_node_port_exists(&self, target_node: NodeIndex, target_port: &str) -> bool {
        self.0
            .edges_directed(target_node, petgraph::Direction::Incoming)
            .any(|e| e.weight().target_port() == target_port)
    }
    pub fn node(&self, uuid: Uuid) -> Option<OpticRef> {
        self.0
            .node_weights()
            .find(|node| node.uuid() == uuid)
            .cloned()
    }
    pub fn node_idx(&self, uuid: Uuid) -> Option<NodeIndex> {
        self.0
            .node_indices()
            .find(|idx| self.0.node_weight(*idx).unwrap().uuid() == uuid)
    }
    pub fn contains_detector(&self) -> bool {
        self.0
            .node_weights()
            .any(|node| node.optical_ref.borrow().is_detector())
    }
    pub fn is_single_tree(&self) -> bool {
        connected_components(&self.0) == 1
    }
    // fn sources(&self) -> Vec<NodeIndex> {
    //     let mut srcs: Vec<NodeIndex> = Vec::new();
    //     for node in self.0.node_indices() {
    //         if self
    //             .0
    //             .neighbors_directed(node, petgraph::Direction::Incoming)
    //             .count()
    //             .is_zero()
    //         {
    //             srcs.push(node);
    //         }
    //     }
    //     srcs
    // }
    // fn sinks(&self) -> Vec<NodeIndex> {
    //     let mut srcs: Vec<NodeIndex> = Vec::new();
    //     for node in self.0.node_indices() {
    //         if self
    //             .0
    //             .neighbors_directed(node, petgraph::Direction::Outgoing)
    //             .count()
    //             .is_zero()
    //         {
    //             srcs.push(node);
    //         }
    //     }
    //     srcs
    // }
    pub fn is_src_node(&self, idx: NodeIndex) -> bool {
        let group_srcs = self.0.externals(petgraph::Direction::Incoming);
        group_srcs.into_iter().any(|gs| gs == idx)
    }
    pub fn is_sink_node(&self, idx: NodeIndex) -> bool {
        let group_sinks = self.0.externals(petgraph::Direction::Outgoing);
        group_sinks.into_iter().any(|gs| gs == idx)
    }
    pub fn calc_node_isometry(&self, node_idx: NodeIndex) -> Option<Isometry> {
        let mut neighbors = self
            .0
            .neighbors_directed(node_idx, petgraph::Direction::Incoming);
        if let Some(neighbor) = neighbors.next() {
            let neighbor_node_ref = self.0.node_weight(neighbor).unwrap();
            let neighbor_node = neighbor_node_ref.optical_ref.borrow();
            let connecting_edge = self.0.edges_connecting(neighbor, node_idx).next().unwrap();
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
    // pub fn update_node_positions(&mut self) {
    //     // iterate over all possible paths from a src to a sink
    //     for src in &self.sources() {
    //         for sink in &self.sinks() {
    //             let paths = algo::all_simple_paths::<Vec<_>, _>(&self.0, *src, *sink, 0, None)
    //                 .collect::<Vec<_>>();
    //             for path in paths {
    //                 for node_idx in path {
    //                     let cloned_graph = self.0.clone();
    //                     // incoming meighbors
    //                     let neighbors: Vec<NodeIndex> = cloned_graph
    //                         .neighbors_directed(node_idx, petgraph::Direction::Incoming)
    //                         .collect();
    //                     for neighbor in neighbors {
    //                         let neighbor_node_ref = cloned_graph.node_weight(neighbor).unwrap();
    //                         let neighbor_node = neighbor_node_ref.optical_ref.borrow();
    //                         let neighbor_name = neighbor_node.name();
    //                         let connecting_edge = cloned_graph
    //                             .edges_connecting(neighbor, node_idx)
    //                             .next()
    //                             .unwrap();
    //                         let connecting_isometery = connecting_edge.weight().isometry();
    //                         let node = self.0.node_weight_mut(node_idx).unwrap();
    //                         if let Some(neighbor_isometry) = neighbor_node.isometry() {
    //                             node.optical_ref
    //                                 .lock()
    //                                 .unwrap()
    //                                 .set_isometry(neighbor_isometry.append(connecting_isometery));
    //                         } else {
    //                             warn!("could not assign node isometry to {} because predecessor node {} has no isometry defined.", node.optical_ref.borrow().name(), neighbor_name);
    //                         }
    //                     }
    //                 }
    //             }
    //         }
    //     }
    // }
}
impl Serialize for OpticGraph {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let g = self.0.clone();
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
                    g.0.add_node(node.clone());
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
                        let Some(reference_node) = g.node(uuid) else {
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
                    let src_idx = g.node_idx(edge.0).ok_or_else(|| {
                        de::Error::custom(format!("src id {} does not exist", edge.0))
                    })?;
                    let target_idx = g.node_idx(edge.1).ok_or_else(|| {
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
    use crate::nodes::Dummy;
    use num::Zero;

    #[test]
    fn add_node() {
        let mut graph = OpticGraph::default();
        graph.add_node(Dummy::new("n1"));
        assert_eq!(graph.0.node_count(), 1);
    }
    #[test]
    fn connect_nodes_ok() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::new("Test"));
        let n2 = graph.add_node(Dummy::new("Test"));
        assert!(graph
            .connect_nodes(n1, "rear", n2, "front", Length::zero())
            .is_ok());
        assert_eq!(graph.0.edge_count(), 1);
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
        assert_eq!(graph.0.edge_count(), 1);
    }
    #[test]
    fn node() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default());
        let uuid = graph.0.node_weight(n1).unwrap().uuid();
        assert!(graph.node(uuid).is_some());
        assert!(graph.node(Uuid::new_v4()).is_none());
    }
    #[test]
    fn node_id() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default());
        let uuid = graph.0.node_weight(n1).unwrap().uuid();
        assert_eq!(graph.node_idx(uuid), Some(n1));
        assert_eq!(graph.node_idx(Uuid::new_v4()), None);
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
