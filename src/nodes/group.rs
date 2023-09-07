#![warn(missing_docs)]
use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::OpossumError;
use crate::light::Light;
use crate::lightdata::LightData;
use crate::optical::{LightResult};
use crate::{
    optical::Optical,
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
/// All unconnected input and output ports of this subgraph could be used as ports of
/// this [`NodeGroup`]. For this, port mapping is neccessary (see below).
///
/// ## Optical Ports
///   - Inputs
///     - defined by [`map_input_port`](NodeGroup::map_input_port()) function.
///   - Outputs
///     - defined by [`map_output_port`](NodeGroup::map_output_port()) function.
pub struct NodeGroup {
    g: DiGraph<Rc<RefCell<dyn Optical>>, Light>,
    expand_view: bool,
    input_port_map: HashMap<String, (NodeIndex, String)>,
    output_port_map: HashMap<String, (NodeIndex, String)>,
    is_inverted: bool,
}

impl NodeGroup {
    /// Creates a new [`NodeGroup`].
    pub fn new() -> Self {
        Self {
            expand_view: false,
            ..Default::default()
        }
    }
    /// Add a given [`OpticNode`] to the (sub-)graph of this [`NodeGroup`].
    ///
    /// This command just adds an [`OpticNode`] but does not connect it to existing nodes in the (sub-)graph. The given node is
    /// consumed (owned) by the [`NodeGroup`].
    pub fn add_node<T: Optical + 'static>(&mut self, node: T) -> NodeIndex {
        self.g.add_node(Rc::new(RefCell::new(node)))
    }
    /// Connect (already existing) nodes denoted by the respective `NodeIndex`.
    ///
    /// Both node indices must exist. Otherwise an [`OpossumError::OpticScenery`] is returned. In addition, connections are
    /// rejected and an [`OpossumError::OpticScenery`] is returned, if the graph would form a cycle (loop in the graph). **Note**:
    /// The connection of two internal nodes might affect external port mappings (see [`map_input_port`](NodeGroup::map_input_port())
    /// & [`map_output_port`](NodeGroup::map_output_port()) functions). In this case no longer valid mappings will be deleted.
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
        let in_map = self.input_port_map.clone();
        let invalid_mapping = in_map
            .iter()
            .find(|m| m.1 .0 == target_node && m.1 .1 == target_port);
        if let Some(input) = invalid_mapping {
            self.input_port_map.remove(input.0);
        }
        let out_map = self.output_port_map.clone();
        let invalid_mapping = out_map
            .iter()
            .find(|m| m.1 .0 == src_node && m.1 .1 == src_port);
        if let Some(input) = invalid_mapping {
            self.output_port_map.remove(input.0);
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
    fn input_nodes(&self) -> Vec<NodeIndex> {
        let mut input_nodes: Vec<NodeIndex> = Vec::default();
        for node_idx in self.g.node_indices() {
            let incoming_edges = self.g.edges_directed(node_idx, Direction::Incoming).count();
            let input_ports = self
                .g
                .node_weight(node_idx)
                .unwrap()
                .borrow()
                .ports()
                .inputs()
                .len();
            if input_ports != incoming_edges {
                input_nodes.push(node_idx);
            }
        }
        input_nodes
    }
    fn output_nodes(&self) -> Vec<NodeIndex> {
        let mut output_nodes: Vec<NodeIndex> = Vec::default();
        for node_idx in self.g.node_indices() {
            let outgoing_edges = self.g.edges_directed(node_idx, Direction::Outgoing).count();
            let output_ports = self
                .g
                .node_weight(node_idx)
                .unwrap()
                .borrow()
                .ports()
                .outputs()
                .len();
            if output_ports != outgoing_edges {
                output_nodes.push(node_idx);
            }
        }
        output_nodes
    }
    /// Map an input port of an internal node to an external port of the group.
    ///
    /// In oder to use a [`NodeGroup`] from the outside, internal nodes / ports must be mapped to be visible. The
    /// corresponding [`ports`](NodeGroup::ports()) function only returns ports that have been mapped before.
    /// # Errors
    ///
    /// This function will return an error if
    ///   - an external input port name has already been assigned.
    ///   - the `input_node` / `internal_name` does not exist.
    ///   - the specified `input_node` is not an input node of the group (i.e. fully connected to other internal nodes).
    ///   - the `input_node` has an input port with the specified `internal_name` but is already internally connected.
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
        if !self.input_nodes().contains(&input_node) {
            return Err(OpossumError::OpticGroup(
                "node to be mapped is not an input node of the group".into(),
            ));
        }
        let incoming_edge_connected = self
            .g
            .edges_directed(input_node, Direction::Incoming)
            .map(|e| e.weight().target_port())
            .any(|p| p == internal_name);
        if incoming_edge_connected {
            return Err(OpossumError::OpticGroup(
                "port of input node is already internally connected".into(),
            ));
        }
        self.input_port_map.insert(
            external_name.to_string(),
            (input_node, internal_name.to_string()),
        );
        Ok(())
    }

    /// Map an output port of an internal node to an external port of the group.
    ///
    /// In oder to use a [`NodeGroup`] from the outside, internal nodes / ports must be mapped to be visible. The
    /// corresponding [`ports`](NodeGroup::ports()) function only returns ports that have been mapped before.
    /// # Errors
    ///
    /// This function will return an error if
    ///   - an external output port name has already been assigned.
    ///   - the `output_node` / `internal_name` does not exist.
    ///   - the specified `output_node` is not an output node of the group (i.e. fully connected to other internal nodes).
    ///   - the `output_node` has an output port with the specified `internal_name` but is already internally connected.
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
        if !self.output_nodes().contains(&output_node) {
            return Err(OpossumError::OpticGroup(
                "node to be mapped is not an output node of the group".into(),
            ));
        }
        let outgoing_edge_connected = self
            .g
            .edges_directed(output_node, Direction::Outgoing)
            .map(|e| e.weight().src_port())
            .any(|p| p == internal_name);
        if outgoing_edge_connected {
            return Err(OpossumError::OpticGroup(
                "port of output node is already internally connected".into(),
            ));
        }
        self.output_port_map.insert(
            external_name.to_string(),
            (output_node, internal_name.to_string()),
        );
        Ok(())
    }
    fn incoming_edges(&self, idx: NodeIndex) -> LightResult {
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
    fn analyze_group(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> Result<LightResult> {
        if self.is_inverted {self.invert_graph();}
        let g_clone = self.g.clone();
        let mut group_srcs = g_clone.externals(Direction::Incoming);
        let mut light_result = LightResult::default();
        let sorted = toposort(&g_clone, None).unwrap();
        for idx in sorted {
            // Check if node is group src node
            let incoming_edges = if group_srcs.any(|gs| gs == idx) {
                // get from incoming_data
                let portmap = if self.is_inverted { &self.output_port_map} else { &self.input_port_map};
                let assigned_ports = portmap.iter().filter(|p| p.1 .0 == idx);
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
            let node = g_clone.node_weight(idx).unwrap();
            let outgoing_edges: HashMap<String, Option<LightData>> = HashMap::new();
            
            //node.borrow().analyze(incoming_edges, analyzer_type)?;
            let mut group_sinks = g_clone.externals(Direction::Outgoing);
            // Check if node is group sink node
            if group_sinks.any(|gs| gs == idx) {
                let portmap = if self.is_inverted { &self.input_port_map} else { &self.output_port_map};
                let assigned_ports = portmap.iter().filter(|p| p.1 .0 == idx);
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
        if self.is_inverted {self.invert_graph();} // revert initial inversion (if necessary)
        Ok(light_result)
    }

    /// Sets the expansion flag of this [`NodeGroup`].  
    /// If true, the group expands and the internal nodes of this group are displayed in the dot format.
    /// If false, only the group node itself is displayed and the internal setup is not shown
    pub fn shall_expand(&self) -> bool {
        self.expand_view
    }

    /// Defines and returns the node/port identifier to connect the edges in the dot format
    /// parameters:
    /// - port_name:            name of the external port of the group
    /// - parent_identifier:    String that contains the hierarchical structure: parentidx_childidx_childofchildidx ...
    ///
    /// Error, if the port is not mapped as input or output
    pub fn get_mapped_port_str(
        &self,
        port_name: &str,
        parent_identifier: String,
    ) -> Result<String> {
        if self.shall_expand() {
            if self.input_port_map.contains_key(port_name) {
                let port = self.input_port_map.get(port_name).unwrap();

                Ok(format!(
                    "{}_i{}:{}",
                    parent_identifier,
                    port.0.index(),
                    port.1
                ))
            } else if self.output_port_map.contains_key(port_name) {
                let port = self.output_port_map.get(port_name).unwrap();

                Ok(format!(
                    "{}_i{}:{}",
                    parent_identifier,
                    port.0.index(),
                    port.1
                ))
            } else {
                Err(OpossumError::OpticGroup(format!(
                    "port {} is not mapped",
                    port_name
                )))
            }
        } else {
            Ok(format!("{}:{}", parent_identifier, port_name))
        }
    }

    /// returns the boolean which defines whether the group expands or not.
    pub fn expand_view(&mut self, expand_view: bool) {
        self.expand_view = expand_view;
    }

    /// downcasts this "OpticNode" with trait "OpicComponent" to its actual struct format "NodeGroup"
    /// parameters:
    /// - ref_node:     reference to the borrowed node of a graph
    ///
    /// Returns a reference to the NodeGroup struct
    ///
    /// Error, if the OpticNode can not be casted to the type of NodeGroup
    // fn cast_node_to_group<'a>(&self, ref_node: &'a dyn Optical) -> Result<&'a NodeGroup> {
    //     let node_boxed = &*ref_node;
    //     let downcasted_node = node_boxed.downcast_ref::<NodeGroup>();

    //     match downcasted_node {
    //         Some(i) => Ok(i),
    //         _ => Err(OpossumError::OpticScenery(
    //             "can not cast OpticNode to specific type of NodeGroup!".into(),
    //         )),
    //     }
    // }

    /// checks if the contained node is a group_node itself.
    /// Returns true, if the node is a group
    /// Returns false otherwise
    fn check_if_group(&self, node_ref: &dyn Optical) -> bool {
        if node_ref.node_type() == "group" {
            true
        } else {
            false
        }
    }

    /// Creates the dot-format string which describes the edge that connects two nodes
    /// parameters:
    /// - end_node_idx:         NodeIndex of the node that should be connected
    /// - light_port:           port name that should be connected
    /// - parent_identifier:    String that contains the hierarchical structure: parentidx_childidx_childofchildidx ...
    ///
    /// Returns the result of the edge strnig for the dot format
    fn create_node_edge_str(
        &self,
        end_node_idx: NodeIndex,
        light_port: &str,
        mut parent_identifier: String,
    ) -> Result<String> {
        let node = self.g.node_weight(end_node_idx).unwrap().borrow();

        parent_identifier = if parent_identifier == "" {
            format!("i{}", end_node_idx.index())
        } else {
            format!("{}_i{}", &parent_identifier, end_node_idx.index())
        };

        // if self.check_if_group(&node) {
        //     let group_node = self.cast_node_to_group(&node)?;
        //     Ok(group_node.get_mapped_port_str(light_port, parent_identifier)?)
        // } else {
            Ok(format!("{}:{}", parent_identifier, light_port))
        // }
    }

    /// creates the dot format of the group node in its expanded view
    /// parameters:
    /// - node_index:           NodeIndex of the group
    /// - name:                 name of the node
    /// - inverted:             boolean that descries wether the node is inverted or not
    /// - parent_identifier:    String that contains the hierarchical structure: parentidx_childidx_childofchildidx ...
    ///
    /// Returns the result of the dot string that describes this node
    fn to_dot_expanded_view(
        &self,
        node_index: &str,
        name: &str,
        inverted: bool,
        mut parent_identifier: String,
    ) -> Result<String> {
        let inv_string = if inverted { "(inv)" } else { "" };
        parent_identifier = if parent_identifier == "" {
            format!("i{}", node_index)
        } else {
            format!("{}_i{}", &parent_identifier, node_index)
        };
        let mut dot_string = format!(
            "  subgraph {} {{\n\tlabel=\"{}{}\"\n\tfontsize=15\n\tcluster=true\n\t",
            parent_identifier, name, inv_string
        );

        // for node_idx in self.g.node_indices() {
        //     let node = self.g.node_weight(node_idx).unwrap();
        //     dot_string += &node
        //         .borrow()
        //         .to_dot(&format!("{}", node_idx.index()), parent_identifier.clone())?;
        // }
        for edge in self.g.edge_indices() {
            let light: &Light = self.g.edge_weight(edge).unwrap();
            let end_nodes = self.g.edge_endpoints(edge).unwrap();

            let src_edge_str = self.create_node_edge_str(
                end_nodes.0,
                light.src_port(),
                parent_identifier.clone(),
            )?;
            let target_edge_str = self.create_node_edge_str(
                end_nodes.1,
                light.target_port(),
                parent_identifier.clone(),
            )?;

            dot_string.push_str(&format!("  {} -> {} \n", src_edge_str, target_edge_str));
            // needed when multiple ports can be assigned
            // for src in src_edge_str.iter(){
            //     println!("{}", src);
            //     for target in target_edge_str.iter(){
            //         println!("{}", target);
            //         dot_string.push_str(&format!("  {} -> {} \n", src, target));
            //     };
            // };
        }
        dot_string += "}";
        Ok(dot_string)
    }

    /// creates the dot format of the group node in its collapsed view
    /// parameters:
    /// - node_index:           NodeIndex of the group
    /// - name:                 name of the node
    /// - inverted:             boolean that descries wether the node is inverted or not
    /// - _ports:               
    /// - parent_identifier:    String that contains the hierarchical structure: parentidx_childidx_childofchildidx ...
    ///
    /// Returns the result of the dot string that describes this node
    fn to_dot_collapsed_view(
        &self,
        node_index: &str,
        name: &str,
        inverted: bool,
        _ports: &OpticPorts,
        mut parent_identifier: String,
    ) -> Result<String> {
        let inv_string = if inverted { " (inv)" } else { "" };
        let node_name = format!("{}{}", name, inv_string);
        parent_identifier = if parent_identifier == "" {
            format!("i{}", node_index)
        } else {
            format!("{}_i{}", &parent_identifier, node_index)
        };
        let mut dot_str = format!("\t{} [\n\t\tshape=plaintext\n", parent_identifier);
        let mut indent_level = 2;
        dot_str.push_str(&self.add_html_like_labels(
            &node_name,
            &mut indent_level,
            _ports,
            inverted,
        ));
        Ok(dot_str)
    }
    fn invert_graph(&mut self) {
        for node in self.g.node_weights_mut() {
            node.borrow_mut().set_inverted(true);
        }
        for edge in self.g.edge_weights_mut() {
            edge.inverse();
        }
        self.g.reverse();
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
    fn set_inverted(&mut self, inverted: bool) {
        self.is_inverted = inverted;
    }
}

impl Dottable for NodeGroup {
    fn to_dot(
        &self,
        node_index: &str,
        name: &str,
        inverted: bool,
        _ports: &OpticPorts,
        parent_identifier: String,
    ) -> Result<String> {
        let mut cloned_self=self.clone();
        if self.is_inverted {cloned_self.invert_graph();}
        if self.expand_view {
            cloned_self.to_dot_expanded_view(node_index, name, inverted, parent_identifier)
        } else {
            cloned_self.to_dot_collapsed_view(node_index, name, inverted, _ports, parent_identifier)
        }
    }

    fn node_color(&self) -> &str {
        "yellow"
    }
}

#[cfg(test)]
mod test {
    use super::NodeGroup;
    use crate::{
        nodes::{BeamSplitter, Dummy},
        optical::Optical,
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
        og.add_node(Dummy::new("n1"));
        assert_eq!(og.g.node_count(), 1);
    }
    #[test]
    fn connect_nodes() {
        let mut og = NodeGroup::new();
        let sn1_i = og.add_node(Dummy::new("n1"));
        let sn2_i = og.add_node(Dummy::new("n2"));
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
    fn connect_nodes_update_port_mapping() {
        let mut og = NodeGroup::new();
        let sn1_i = og.add_node(Dummy::new("n1"));
        let sn2_i = og.add_node(Dummy::new("n2"));

        og.map_input_port(sn2_i, "front", "input").unwrap();
        og.map_output_port(sn1_i, "rear", "output").unwrap();
        assert_eq!(og.input_port_map.len(), 1);
        assert_eq!(og.output_port_map.len(), 1);
        og.connect_nodes(sn1_i, "rear", sn2_i, "front").unwrap();
        // delete no longer valid port mapping
        assert_eq!(og.input_port_map.len(), 0);
        assert_eq!(og.output_port_map.len(), 0);
    }
    #[test]
    fn input_nodes() {
        let mut og = NodeGroup::new();
        let sn1_i = og.add_node(Dummy::new("n1"));
        let sn2_i = og.add_node(Dummy::new("n2"));
        let sub_node3 = BeamSplitter::new(0.5).unwrap();
        let sn3_i = og.add_node(sub_node3);
        og.connect_nodes(sn1_i, "rear", sn2_i, "front").unwrap();
        og.connect_nodes(sn2_i, "rear", sn3_i, "input1").unwrap();
        assert_eq!(og.input_nodes(), vec![0.into(), 2.into()])
    }
    #[test]
    fn output_nodes() {
        let mut og = NodeGroup::new();
        let sn1_i = og.add_node(Dummy::new("n1"));
        let sub_node1 = BeamSplitter::new(0.5).unwrap();
        let sn2_i = og.add_node(sub_node1);
        let sn3_i = og.add_node(Dummy::new("n3"));
        og.connect_nodes(sn1_i, "rear", sn2_i, "input1").unwrap();
        og.connect_nodes(sn2_i, "out1_trans1_refl2", sn3_i, "front")
            .unwrap();
        assert_eq!(og.input_nodes(), vec![0.into(), 1.into()])
    }
    #[test]
    fn map_input_port() {
        let mut og = NodeGroup::new();
        let sn1_i = og.add_node(Dummy::new("n1"));
        let sn2_i = og.add_node(Dummy::new("n2"));
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
    fn map_input_port_half_connected_nodes() {
        let mut og = NodeGroup::new();
        let sn1_i = og.add_node(Dummy::new("n1"));
        let sn2_i = og.add_node(Dummy::new("n2"));
        og.connect_nodes(sn1_i, "rear", sn2_i, "input1").unwrap();

        // node port already internally connected
        assert!(og.map_input_port(sn2_i, "input1", "bs_input").is_err());

        // correct usage
        assert!(og.map_input_port(sn1_i, "front", "input").is_ok());
        assert!(og.map_input_port(sn2_i, "input2", "bs_input").is_ok());
        assert_eq!(og.input_port_map.len(), 2);
    }
    #[test]
    fn map_output_port() {
        let mut og = NodeGroup::new();
        let sn1_i = og.add_node(Dummy::new("n1"));
        let sn2_i = og.add_node(Dummy::new("n2"));
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
    fn map_output_port_half_connected_nodes() {
        let mut og = NodeGroup::new();
        let sn1_i = og.add_node(BeamSplitter::default());
        let sn2_i = og.add_node(Dummy::new("n2"));
        og.connect_nodes(sn1_i, "out1_trans1_refl2", sn2_i, "front")
            .unwrap();

        // node port already internally connected
        assert!(og
            .map_output_port(sn1_i, "out1_trans1_refl2", "bs_output")
            .is_err());

        // correct usage
        assert!(og
            .map_output_port(sn1_i, "out2_trans2_refl1", "bs_output")
            .is_ok());
        assert!(og.map_output_port(sn2_i, "rear", "output").is_ok());
        assert_eq!(og.output_port_map.len(), 2);
    }
    #[test]
    fn ports() {
        let mut og = NodeGroup::new();
        let sn1_i = og.add_node(Dummy::new("n1"));
        let sn2_i = og.add_node(Dummy::new("n2"));
        og.connect_nodes(sn1_i, "rear", sn2_i, "front").unwrap();
        assert!(og.ports().inputs().is_empty());
        assert!(og.ports().outputs().is_empty());
        og.map_input_port(sn1_i, "front", "input").unwrap();
        assert!(og.ports().inputs().contains(&("input".to_string())));
        og.map_output_port(sn2_i, "rear", "output").unwrap();
        assert!(og.ports().outputs().contains(&("output".to_string())));
    }
}
