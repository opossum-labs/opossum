use std::{cell::RefCell, rc::Rc};

use petgraph::{algo::is_cyclic_directed, prelude::DiGraph, stable_graph::NodeIndex};
use serde::{
    de::{self, MapAccess, Visitor},
    ser::SerializeStruct,
    Deserialize, Serialize,
};
use uuid::Uuid;

use crate::optic_ref::OpticRef;
use crate::{
    error::{OpmResult, OpossumError},
    light::Light,
    optical::Optical,
};

#[derive(Debug, Default, Clone)]
pub struct OpticGraph(pub DiGraph<OpticRef, Light>);

impl OpticGraph {
    pub fn add_node<T: Optical + 'static>(&mut self, node: T) -> NodeIndex {
        self.0.add_node(OpticRef::new(Rc::new(RefCell::new(node)), None))
    }

    pub fn connect_nodes(
        &mut self,
        src_node: NodeIndex,
        src_port: &str,
        target_node: NodeIndex,
        target_port: &str,
    ) -> OpmResult<()> {
        let source = self
            .0
            .node_weight(src_node)
            .ok_or(OpossumError::OpticScenery(
                "source node with given index does not exist".into(),
            ))?;
        if !source
            .optical_ref
            .borrow()
            .ports()
            .outputs()
            .contains(&src_port.into())
        {
            return Err(OpossumError::OpticScenery(format!(
                "source node {} does not have a port {}",
                source.optical_ref.borrow().name(),
                src_port
            )));
        }
        let target = self
            .0
            .node_weight(target_node)
            .ok_or(OpossumError::OpticScenery(
                "target node with given index does not exist".into(),
            ))?;
        if !target
            .optical_ref
            .borrow()
            .ports()
            .inputs()
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
        let src_name = source.optical_ref.borrow().name().to_owned();
        let target_name = target.optical_ref.borrow().name().to_owned();
        let edge_index = self
            .0
            .add_edge(src_node, target_node, Light::new(src_port, target_port));
        if is_cyclic_directed(&self.0) {
            self.0.remove_edge(edge_index);
            return Err(OpossumError::OpticScenery(format!(
                "connecting nodes <{}> -> <{}> would form a loop",
                src_name, target_name
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
        self.0.node_weights().find(|node| node.uuid()==uuid).cloned()
    }
    pub fn node_idx(&self, uuid: Uuid) -> NodeIndex {
        self.0.node_indices().find(|idx| self.0.node_weight(*idx).unwrap().uuid()==uuid).unwrap()
    }
}
impl Serialize for OpticGraph {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let g = self.0.clone();
        let mut graph = serializer.serialize_struct("graph", 2)?;
        let nodes = g
            .node_weights()
            .map(|n| n.to_owned())
            .collect::<Vec<OpticRef>>();
        graph.serialize_field("nodes", &nodes)?;
        let edgeidx = g
            .edge_indices()
            .map(|e| {
                (
                    g.node_weight(g.edge_endpoints(e).unwrap().0).unwrap().uuid(),
                    g.node_weight(g.edge_endpoints(e).unwrap().1).unwrap().uuid(),
                    g.edge_weight(e).unwrap().src_port(),
                    g.edge_weight(e).unwrap().target_port(),
                )
            })
            .collect::<Vec<(Uuid, Uuid, &str, &str)>>();
        graph.serialize_field("edges", &edgeidx)?;
        graph.end()
    }
}

impl<'de> Deserialize<'de> for OpticGraph {
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

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
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

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an OpticGraph")
            }
            // fn visit_seq<A>(self, mut seq: A) -> std::result::Result<OpticGraph, A::Error>
            // where
            //     A: SeqAccess<'de>,
            // {
            //     println!("visit seq");
            //     let g = OpticGraph::default();
            //     Ok(g)
            // }
            fn visit_map<A>(self, mut map: A) -> std::result::Result<OpticGraph, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut g = OpticGraph::default();
                let mut nodes: Option<Vec<OpticRef>> = None;
                let mut edges: Option<Vec<(Uuid, Uuid, &str, &str)>> = None;
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
                            edges =
                                Some(map.next_value::<Vec<(Uuid, Uuid, &str, &str)>>()?);
                        }
                    }
                }
                let nodes = nodes.ok_or_else(|| de::Error::missing_field("nodes"))?;
                let edges = edges.ok_or_else(|| de::Error::missing_field("edges"))?;
                for node in nodes.iter() {
                    g.0.add_node(node.clone());
                }
                for edge in edges.iter() {
                    let src_idx=g.node_idx(edge.0);
                    let target_idx=g.node_idx(edge.1);
                    g.connect_nodes(src_idx, edge.2, target_idx, edge.3)
                        .map_err(|e| {
                            de::Error::custom(format!("connecting OpticGraph nodes failed: {}", e))
                        })?;
                }
                Ok(g)
            }
        }
        deserializer.deserialize_struct("OpticGraph", FIELDS, OpticGraphVisitor)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::nodes::Dummy;
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
        assert!(graph.connect_nodes(n1, "rear", n2, "front").is_ok());
        assert_eq!(graph.0.edge_count(), 1);
    }
    #[test]
    fn connect_nodes_failure() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::new("Test"));
        let n2 = graph.add_node(Dummy::new("Test"));
        assert!(graph
            .connect_nodes(n1, "rear", NodeIndex::new(5), "front")
            .is_err());
        assert!(graph
            .connect_nodes(NodeIndex::new(5), "rear", n2, "front")
            .is_err());
    }
    #[test]
    fn connect_nodes_target_already_connected() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::new("Test"));
        let n2 = graph.add_node(Dummy::new("Test"));
        let n3 = graph.add_node(Dummy::new("Test"));
        assert!(graph.connect_nodes(n1, "rear", n2, "front").is_ok());
        assert!(graph.connect_nodes(n3, "rear", n2, "front").is_err());
    }
    #[test]
    fn connect_nodes_loop_error() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::new("Test"));
        let n2 = graph.add_node(Dummy::new("Test"));
        assert!(graph.connect_nodes(n1, "rear", n2, "front").is_ok());
        assert!(graph.connect_nodes(n2, "rear", n1, "front").is_err());
        assert_eq!(graph.0.edge_count(), 1);
    }
}
