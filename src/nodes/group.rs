use crate::analyzer::AnalyzerType;
use crate::error::OpossumError;
use crate::light::Light;
use crate::lightdata::LightData;
use crate::optic_node::{Dottable, LightResult};
use crate::{
    optic_node::{OpticNode, Optical},
    optic_ports::OpticPorts,
};
use petgraph::prelude::{DiGraph, EdgeIndex, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::{algo::*, Direction};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type Result<T> = std::result::Result<T, OpossumError>;

#[derive(Default, Debug, Clone)]
/// A node that represents a group of other [`OpticNode`]s arranges in a subgraph.
///
/// All unconnected input and output ports of this subgraph form the ports of this [`NodeGroup`].
pub struct NodeGroup {
    g: DiGraph<Rc<RefCell<OpticNode>>, Light>,
    input_port_map: HashMap<String, (NodeIndex, String)>,
    output_port_map: HashMap<String, (NodeIndex, String)>,
}

impl NodeGroup {
    pub fn new() -> Self {
        Self::default()
    }
    /// Add a given [`OpticNode`] to the (sub-)graph of this [`NodeGroup`].
    ///
    /// This command just adds an [`OpticNode`] but does not connect it to existing nodes in the (sub-)graph. The given node is
    /// consumed (owned) by the [`NodeGroup`].
    pub fn add_node(&mut self, node: OpticNode) -> NodeIndex {
        self.g.add_node(Rc::new(RefCell::new(node)))
    }
    /// Connect (already existing) nodes denoted by the respective `NodeIndex`.
    ///
    /// Both node indices must exist. Otherwise an [`OpossumError::OpticScenery`] is returned. In addition, connections are
    /// rejected and an [`OpossumError::OpticScenery`] is returned, if the graph would form a cycle (loop in the graph).
    pub fn connect_nodes(
        &mut self,
        src_node: NodeIndex,
        src_port: &str,
        target_node: NodeIndex,
        target_port: &str,
    ) -> Result<EdgeIndex> {
        if let Some(source) = self.g.node_weight(src_node) {
            if !source.borrow().ports().outputs().contains(&src_port.into()) {
                return Err(OpossumError::OpticScenery(format!(
                    "source node {} does not have a port {}",
                    source.borrow().name(),
                    src_port
                )));
            }
        } else {
            return Err(OpossumError::OpticScenery(
                "source node with given index does not exist".into(),
            ));
        }
        if let Some(target) = self.g.node_weight(target_node) {
            if !target
                .borrow()
                .ports()
                .inputs()
                .contains(&target_port.into())
            {
                return Err(OpossumError::OpticScenery(format!(
                    "target node {} does not have a port {}",
                    target.borrow().name(),
                    target_port
                )));
            }
        } else {
            return Err(OpossumError::OpticScenery(
                "target node with given index does not exist".into(),
            ));
        }
        if self.src_node_port_exists(src_node, src_port) {
            return Err(OpossumError::OpticScenery(format!(
                "src node with given port {} is already connected",
                src_port
            )));
        }
        if self.target_node_port_exists(src_node, src_port) {
            return Err(OpossumError::OpticScenery(format!(
                "target node with given port {} is already connected",
                target_port
            )));
        }
        let edge_index = self
            .g
            .add_edge(src_node, target_node, Light::new(src_port, target_port));
        if is_cyclic_directed(&self.g) {
            self.g.remove_edge(edge_index);
            return Err(OpossumError::OpticScenery(
                "connecting the given nodes would form a loop".into(),
            ));
        }
        Ok(edge_index)
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
    pub fn map_input_port(
        &mut self,
        input_node: NodeIndex,
        internal_name: &str,
        external_name: &str,
    ) -> Result<()> {
        if self.input_port_map.contains_key(external_name) {
            return Err(OpossumError::OpticGroup(
                "external input port name already assigned".into(),
            ));
        }
        if let Some(node) = self.g.node_weight(input_node) {
            if !node
                .borrow()
                .ports()
                .inputs()
                .contains(&(internal_name.to_string()))
            {
                return Err(OpossumError::OpticGroup(
                    "internal input port name not found".into(),
                ));
            }
        } else {
            return Err(OpossumError::OpticGroup(
                "internal node index not found".into(),
            ));
        }

        if !(self
            .g
            .externals(petgraph::Direction::Incoming)
            .any(|i| i == input_node))
        {
            return Err(OpossumError::OpticGroup(
                "node to be mapped is not an input node of the group".into(),
            ));
        }
        self.input_port_map.insert(
            external_name.to_string(),
            (input_node, internal_name.to_string()),
        );
        Ok(())
    }
    pub fn map_output_port(
        &mut self,
        output_node: NodeIndex,
        internal_name: &str,
        external_name: &str,
    ) -> Result<()> {
        if self.output_port_map.contains_key(external_name) {
            return Err(OpossumError::OpticGroup(
                "external output port name already assigned".into(),
            ));
        }
        if let Some(node) = self.g.node_weight(output_node) {
            if !node
                .borrow()
                .ports()
                .outputs()
                .contains(&(internal_name.to_string()))
            {
                return Err(OpossumError::OpticGroup(
                    "internal output port name not found".into(),
                ));
            }
        } else {
            return Err(OpossumError::OpticGroup(
                "internal node index not found".into(),
            ));
        }
        if !(self
            .g
            .externals(petgraph::Direction::Outgoing)
            .any(|i| i == output_node))
        {
            return Err(OpossumError::OpticGroup(
                "node to be mapped is not an output node of the group".into(),
            ));
        }
        self.output_port_map.insert(
            external_name.to_string(),
            (output_node, internal_name.to_string()),
        );
        Ok(())
    }
    pub fn incoming_edges(&self, idx: NodeIndex) -> LightResult {
        let edges = self.g.edges_directed(idx, Direction::Incoming);
        edges
            .into_iter()
            .map(|e| {
                (
                    e.weight().target_port().to_owned(),
                    e.weight().data().cloned(),
                )
            })
            .collect::<HashMap<String, Option<LightData>>>()
    }
    fn set_outgoing_edge_data(&mut self, idx: NodeIndex, port: String, data: Option<LightData>) {
        let edges = self.g.edges_directed(idx, Direction::Outgoing);
        let edge_ref = edges
            .into_iter()
            .filter(|idx| idx.weight().src_port() == port)
            .last();
        if let Some(edge_ref) = edge_ref {
            let edge_idx = edge_ref.id();
            let light = self.g.edge_weight_mut(edge_idx);
            if let Some(light) = light {
                light.set_data(data);
            }
        } // else outgoing edge not connected -> data dropped
    }
    pub fn analyze_group(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> Result<LightResult> {
        let g_clone = self.g.clone();
        let mut group_srcs = g_clone.externals(Direction::Incoming);
        let mut light_result = LightResult::default();
        let sorted = toposort(&self.g, None).unwrap();
        for idx in sorted {
            // Check if node is group src node
            let incoming_edges = if group_srcs.any(|gs| gs == idx) {
                // get from incoming_data
                let assigned_ports = self.input_port_map.iter().filter(|p| p.1 .0 == idx);
                let mut incoming = LightResult::default();
                for port in assigned_ports {
                    incoming.insert(
                        port.1 .1.to_owned(),
                        incoming_data.get(port.0).unwrap().clone(),
                    );
                }
                incoming
            } else {
                self.incoming_edges(idx)
            };
            let node = self.g.node_weight(idx).unwrap();
            let outgoing_edges = node.borrow_mut().analyze(incoming_edges, analyzer_type)?;
            let mut group_sinks = self.g.externals(Direction::Outgoing);
            // Check if node is group sink node
            if group_sinks.any(|gs| gs == idx) {
                let assigned_ports = self.output_port_map.iter().filter(|p| p.1 .0 == idx);
                for port in assigned_ports {
                    light_result.insert(
                        port.0.to_owned(),
                        outgoing_edges.get(&port.1 .1).unwrap().clone(),
                    );
                }
            } else {
                for outgoing_edge in outgoing_edges {
                    self.set_outgoing_edge_data(idx, outgoing_edge.0, outgoing_edge.1)
                }
            }
        }
        Ok(light_result)
    }
}

impl Optical for NodeGroup {
    fn node_type(&self) -> &str {
        "group"
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        for p in self.input_port_map.iter() {
            ports.add_input(p.0).unwrap();
        }
        for p in self.output_port_map.iter() {
            ports.add_output(p.0).unwrap();
        }
        ports
    }
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> Result<LightResult> {
        self.analyze_group(incoming_data, analyzer_type)
    }
}

impl Dottable for NodeGroup {
    fn to_dot(&self, node_index: &str, name: &str, inverted: bool, _ports: &OpticPorts) -> String {
        let inv_string = if inverted { "(inv)" } else { "" };
        let mut dot_string = format!(
            "  subgraph i{} {{\n\tlabel=\"{}{}\"\n\tfontsize=15\n\tcluster=true\n\t",
            node_index, name, inv_string
        );
        for node_idx in self.g.node_indices() {
            let node = self.g.node_weight(node_idx).unwrap();
            dot_string += &node
                .borrow()
                .to_dot(&format!("{}_i{}", node_index, node_idx.index()));
        }
        for edge in self.g.edge_indices() {
            let end_nodes = self.g.edge_endpoints(edge).unwrap();
            let light = self.g.edge_weight(edge).unwrap();
            dot_string.push_str(&format!(
                "      i{}_i{}:{} -> i{}_i{}:{}\n",
                node_index,
                end_nodes.0.index(),
                light.src_port(),
                node_index,
                end_nodes.1.index(),
                light.target_port()
            ));
        }
        dot_string += "}";
        dot_string
    }

    fn node_color(&self) -> &str {
        "yellow"
    }
}

#[cfg(test)]
mod test {
    use super::NodeGroup;
    use crate::{
        nodes::Dummy,
        optic_node::{OpticNode, Optical},
    };
    #[test]
    fn new() {
        let og = NodeGroup::new();
        assert_eq!(og.g.node_count(), 0);
        assert_eq!(og.g.edge_count(), 0);
        assert!(og.input_port_map.is_empty());
        assert!(og.output_port_map.is_empty());
    }
    #[test]
    fn add_node() {
        let mut og = NodeGroup::new();
        let sub_node = OpticNode::new("test", Dummy);
        og.add_node(sub_node);
        assert_eq!(og.g.node_count(), 1);
    }
    #[test]
    fn connect_nodes() {
        let mut og = NodeGroup::new();
        let sub_node1 = OpticNode::new("test1", Dummy);
        let sn1_i = og.add_node(sub_node1);
        let sub_node2 = OpticNode::new("test2", Dummy);
        let sn2_i = og.add_node(sub_node2);
        // wrong port names
        assert!(og.connect_nodes(sn1_i, "wrong", sn2_i, "front").is_err());
        assert_eq!(og.g.edge_count(), 0);
        assert!(og.connect_nodes(sn1_i, "rear", sn2_i, "wrong").is_err());
        assert_eq!(og.g.edge_count(), 0);
        // wrong node index
        assert!(og.connect_nodes(5.into(), "rear", sn2_i, "front").is_err());
        assert_eq!(og.g.edge_count(), 0);
        assert!(og.connect_nodes(sn1_i, "rear", 5.into(), "front").is_err());
        assert_eq!(og.g.edge_count(), 0);
        // correct usage
        assert!(og.connect_nodes(sn1_i, "rear", sn2_i, "front").is_ok());
        assert_eq!(og.g.edge_count(), 1);
    }
    #[test]
    fn map_input_port() {
        let mut og = NodeGroup::new();
        let sub_node1 = OpticNode::new("test1", Dummy);
        let sn1_i = og.add_node(sub_node1);
        let sub_node2 = OpticNode::new("test2", Dummy);
        let sn2_i = og.add_node(sub_node2);
        og.connect_nodes(sn1_i, "rear", sn2_i, "front").unwrap();

        // wrong port name
        assert!(og.map_input_port(sn1_i, "wrong", "input").is_err());
        assert!(og.input_port_map.is_empty());
        // wrong node index
        assert!(og.map_input_port(5.into(), "front", "input").is_err());
        assert!(og.input_port_map.is_empty());
        // map output port
        assert!(og.map_input_port(sn2_i, "rear", "input").is_err());
        assert!(og.input_port_map.is_empty());
        // map internal node
        assert!(og.map_input_port(sn2_i, "front", "input").is_err());
        assert!(og.input_port_map.is_empty());
        // correct usage
        assert!(og.map_input_port(sn1_i, "front", "input").is_ok());
        assert_eq!(og.input_port_map.len(), 1);
    }
    #[test]
    fn map_output_port() {
        let mut og = NodeGroup::new();
        let sub_node1 = OpticNode::new("test1", Dummy);
        let sn1_i = og.add_node(sub_node1);
        let sub_node2 = OpticNode::new("test2", Dummy);
        let sn2_i = og.add_node(sub_node2);
        og.connect_nodes(sn1_i, "rear", sn2_i, "front").unwrap();

        // wrong port name
        assert!(og.map_output_port(sn2_i, "wrong", "output").is_err());
        assert!(og.output_port_map.is_empty());
        // wrong node index
        assert!(og.map_output_port(5.into(), "rear", "output").is_err());
        assert!(og.output_port_map.is_empty());
        // map input port
        assert!(og.map_output_port(sn1_i, "front", "output").is_err());
        assert!(og.output_port_map.is_empty());
        // map internal node
        assert!(og.map_output_port(sn1_i, "rear", "output").is_err());
        assert!(og.output_port_map.is_empty());
        // correct usage
        assert!(og.map_output_port(sn2_i, "rear", "output").is_ok());
        assert_eq!(og.output_port_map.len(), 1);
    }
    #[test]
    fn ports() {
        let mut og = NodeGroup::new();
        let sub_node1 = OpticNode::new("test1", Dummy);
        let sn1_i = og.add_node(sub_node1);
        let sub_node2 = OpticNode::new("test2", Dummy);
        let sn2_i = og.add_node(sub_node2);
        og.connect_nodes(sn1_i, "rear", sn2_i, "front").unwrap();
        assert!(og.ports().inputs().is_empty());
        assert!(og.ports().outputs().is_empty());
        og.map_input_port(sn1_i, "front", "input").unwrap();
        assert!(og.ports().inputs().contains(&("input".to_string())));
        og.map_output_port(sn2_i, "rear", "output").unwrap();
        assert!(og.ports().outputs().contains(&("output".to_string())));
    }
}
