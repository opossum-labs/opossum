#![warn(missing_docs)]
use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::OpmResult;
use crate::error::OpossumError;
use crate::light::Light;
use crate::lightdata::LightData;
use crate::optic_graph::OpticGraph;
use crate::optical::LightResult;
use crate::properties::{Properties, Proptype};
use crate::reporter::NodeReport;
use crate::{optic_ports::OpticPorts, optical::Optical};
use log::warn;
use petgraph::prelude::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::{algo::toposort, Direction};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

use super::node_attr::NodeAttr;

/// Mapping of group internal [`OpticPorts`] to externally visible ports.
pub type PortMap = HashMap<String, (NodeIndex, String)>;
impl From<PortMap> for Proptype {
    fn from(value: PortMap) -> Self {
        Self::GroupPortMap(value)
    }
}

#[derive(Debug, Clone, Serialize)]
/// A node that represents a group of other [`Optical`]s arranges in a subgraph.
///
/// All unconnected input and output ports of this subgraph could be used as ports of
/// this [`NodeGroup`]. For this, port mapping is neccessary (see below).
///
/// ## Optical Ports
///   - Inputs
///     - defined by [`map_input_port`](NodeGroup::map_input_port()) function.
///   - Outputs
///     - defined by [`map_output_port`](NodeGroup::map_output_port()) function.
///
/// ## Properties
///   - `name`
///   - `inverted`
///   - `expand view`
///   - `graph`
///   - `input port map`
///   - `output port map`
///
/// **Note**: The group node does currently ignore all [`Aperture`](crate::aperture::Aperture) definitions on its publicly
/// mapped input and output ports.
pub struct NodeGroup {
    #[serde(skip)]
    g: OpticGraph,
    node_attr: NodeAttr,
}

impl Default for NodeGroup {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("group", "group");
        node_attr
            .create_property(
                "expand view",
                "show group fully expanded in dot diagram?",
                None,
                false.into(),
            )
            .unwrap();
        node_attr
            .create_property("graph", "optical graph", None, OpticGraph::default().into())
            .unwrap();
        node_attr
            .create_property(
                "input port map",
                "mapping of internal input ports to external ones",
                None,
                PortMap::new().into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "output port map",
                "mapping of internal output ports to external ones",
                None,
                PortMap::new().into(),
            )
            .unwrap();
        Self {
            g: OpticGraph::default(),
            node_attr,
        }
    }
}
impl NodeGroup {
    /// Creates a new [`NodeGroup`].
    /// # Attributes
    /// * `name`: name of the  [`NodeGroup`]
    ///
    /// # Panics
    /// This function panics if
    /// - the property `name` can not be set.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut group = Self::default();
        group.node_attr.set_property("name", name.into()).unwrap();
        group
    }
    /// Add a given [`Optical`] to the (sub-)graph of this [`NodeGroup`].
    ///
    /// This command just adds an [`Optical`] but does not connect it to existing nodes in the (sub-)graph. The given node is
    /// consumed (owned) by the [`NodeGroup`].
    ///
    /// # Errors
    /// An error is returned if the [`NodeGroup`] is set as inverted (which would lead to strange behaviour).
    ///
    /// # Panics
    /// This function panics if the property "graph" can not be unchecked. Produces an error of type [`OpossumError::Properties`]
    pub fn add_node<T: Optical + 'static>(&mut self, node: T) -> OpmResult<NodeIndex> {
        if self.properties().inverted()? {
            return Err(OpossumError::OpticGroup(
                "cannot add nodes if group is set as inverted".into(),
            ));
        }
        let idx = self.g.add_node(node);
        self.node_attr
            .set_property("graph", self.g.clone().into())
            .unwrap();
        Ok(idx)
    }
    /// Connect (already existing) nodes denoted by the respective `NodeIndex`.
    ///
    /// Both node indices must exist. Otherwise an [`OpossumError::OpticScenery`] is returned. In addition, connections are
    /// rejected and an [`OpossumError::OpticScenery`] is returned, if the graph would form a cycle (loop in the graph). **Note**:
    /// The connection of two internal nodes might affect external port mappings (see [`map_input_port`](NodeGroup::map_input_port())
    /// & [`map_output_port`](NodeGroup::map_output_port()) functions). In this case no longer valid mappings will be deleted.
    ///
    /// # Errors
    /// This function returns an [`OpossumError`] if
    ///   - the group is set as `inverted`. Connectiing subnodes of an inverted group node would result in strange behaviour.
    ///   - the source node / port or target node / port does not exist.
    ///   - the source node / port or target node / port is already connected.
    ///   - the node connection would form a loop in the graph.
    ///
    /// # Panics
    /// This function panics if the property "graph" can not be unchecked. Produces an error of type [`OpossumError::Properties`]
    pub fn connect_nodes(
        &mut self,
        src_node: NodeIndex,
        src_port: &str,
        target_node: NodeIndex,
        target_port: &str,
    ) -> OpmResult<()> {
        if self.properties().inverted()? {
            return Err(OpossumError::OpticGroup(
                "cannot connect nodes if group is set as inverted".into(),
            ));
        }
        self.g
            .connect_nodes(src_node, src_port, target_node, target_port)?;
        self.node_attr
            .set_property("graph", self.g.clone().into())
            .unwrap();

        let in_map = self.input_port_map();
        let invalid_mapping = in_map
            .iter()
            .find(|m| m.1 .0 == target_node && m.1 .1 == target_port);
        let mut in_map = self.input_port_map();
        if let Some(input) = invalid_mapping {
            in_map.remove(input.0);
            self.set_input_port_map(in_map);
        }
        let out_map = self.output_port_map();
        let invalid_mapping = out_map
            .iter()
            .find(|m| m.1 .0 == src_node && m.1 .1 == src_port);
        let mut out_map = self.output_port_map();
        if let Some(input) = invalid_mapping {
            out_map.remove(input.0);
            self.set_output_port_map(out_map);
        }
        Ok(())
    }

    fn input_port_map(&self) -> PortMap {
        let input_port_map = self
            .node_attr
            .get_property("input port map")
            .unwrap()
            .clone();
        if let Proptype::GroupPortMap(input_port_map) = input_port_map {
            input_port_map
        } else {
            panic!("wrong data type")
        }
    }
    fn set_input_port_map(&mut self, port_map: PortMap) {
        self.node_attr
            .set_property("input port map", port_map.into())
            .unwrap();
    }
    fn output_port_map(&self) -> PortMap {
        let output_port_map = self
            .node_attr
            .get_property("output port map")
            .unwrap()
            .clone();
        if let Proptype::GroupPortMap(output_port_map) = output_port_map {
            output_port_map
        } else {
            panic!("wrong data type")
        }
    }
    fn set_output_port_map(&mut self, port_map: PortMap) {
        self.node_attr
            .set_property("output port map", port_map.into())
            .unwrap();
    }

    fn input_nodes(&self) -> Vec<NodeIndex> {
        let mut input_nodes: Vec<NodeIndex> = Vec::default();
        for node_idx in self.g.0.node_indices() {
            let incoming_edges = self
                .g
                .0
                .edges_directed(node_idx, Direction::Incoming)
                .count();
            let input_ports = self
                .g
                .0
                .node_weight(node_idx)
                .unwrap()
                .optical_ref
                .borrow()
                .ports()
                .input_names()
                .len();
            if input_ports != incoming_edges {
                input_nodes.push(node_idx);
            }
        }
        input_nodes
    }
    fn output_nodes(&self) -> Vec<NodeIndex> {
        let mut output_nodes: Vec<NodeIndex> = Vec::default();
        for node_idx in self.g.0.node_indices() {
            let outgoing_edges = self
                .g
                .0
                .edges_directed(node_idx, Direction::Outgoing)
                .count();
            let output_ports = self
                .g
                .0
                .node_weight(node_idx)
                .unwrap()
                .optical_ref
                .borrow()
                .ports()
                .output_names()
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
    ) -> OpmResult<()> {
        if self.input_port_map().contains_key(external_name) {
            return Err(OpossumError::OpticGroup(
                "external input port name already assigned".into(),
            ));
        }
        let node = self
            .g
            .0
            .node_weight(input_node)
            .ok_or_else(|| OpossumError::OpticGroup("internal node index not found".into()))?;
        if !node
            .optical_ref
            .borrow()
            .ports()
            .input_names()
            .contains(&(internal_name.to_string()))
        {
            return Err(OpossumError::OpticGroup(
                "internal input port name not found".into(),
            ));
        }
        if !self.input_nodes().contains(&input_node) {
            return Err(OpossumError::OpticGroup(
                "node to be mapped is not an input node of the group".into(),
            ));
        }
        let incoming_edge_connected = self
            .g
            .0
            .edges_directed(input_node, Direction::Incoming)
            .map(|e| e.weight().target_port())
            .any(|p| p == internal_name);
        if incoming_edge_connected {
            return Err(OpossumError::OpticGroup(
                "port of input node is already internally connected".into(),
            ));
        }
        let mut input_port_map = self.input_port_map();
        input_port_map.insert(
            external_name.to_string(),
            (input_node, internal_name.to_string()),
        );
        self.set_input_port_map(input_port_map);
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
    ) -> OpmResult<()> {
        if self.output_port_map().contains_key(external_name) {
            return Err(OpossumError::OpticGroup(
                "external output port name already assigned".into(),
            ));
        }
        let node = self
            .g
            .0
            .node_weight(output_node)
            .ok_or_else(|| OpossumError::OpticGroup("internal node index not found".into()))?;
        if !node
            .optical_ref
            .borrow()
            .ports()
            .output_names()
            .contains(&(internal_name.to_string()))
        {
            return Err(OpossumError::OpticGroup(
                "internal output port name not found".into(),
            ));
        }

        if !self.output_nodes().contains(&output_node) {
            return Err(OpossumError::OpticGroup(
                "node to be mapped is not an output node of the group".into(),
            ));
        }
        let outgoing_edge_connected = self
            .g
            .0
            .edges_directed(output_node, Direction::Outgoing)
            .map(|e| e.weight().src_port())
            .any(|p| p == internal_name);
        if outgoing_edge_connected {
            return Err(OpossumError::OpticGroup(
                "port of output node is already internally connected".into(),
            ));
        }
        let mut out_map = self.output_port_map();
        out_map.insert(
            external_name.to_string(),
            (output_node, internal_name.to_string()),
        );
        self.set_output_port_map(out_map);
        Ok(())
    }
    fn incoming_edges(&self, idx: NodeIndex) -> LightResult {
        let edges = self.g.0.edges_directed(idx, Direction::Incoming);
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
    fn set_outgoing_edge_data(&mut self, idx: NodeIndex, port: &str, data: Option<LightData>) {
        let edges = self.g.0.edges_directed(idx, Direction::Outgoing);
        let edge_ref = edges
            .into_iter()
            .filter(|idx| idx.weight().src_port() == port)
            .last();
        if let Some(edge_ref) = edge_ref {
            let edge_idx = edge_ref.id();
            let light = self.g.0.edge_weight_mut(edge_idx);
            if let Some(light) = light {
                light.set_data(data);
            }
        } // else outgoing edge not connected -> data dropped
    }
    fn is_group_src_node(&self, idx: NodeIndex) -> bool {
        let group_srcs = self.g.0.externals(Direction::Incoming);
        group_srcs.into_iter().any(|gs| gs == idx)
    }
    fn is_group_sink_node(&self, idx: NodeIndex) -> bool {
        let group_sinks = self.g.0.externals(Direction::Outgoing);
        group_sinks.into_iter().any(|gs| gs == idx)
    }
    fn get_incoming(&self, idx: NodeIndex, incoming_data: &LightResult) -> LightResult {
        if self.is_group_src_node(idx) {
            // get from incoming_data
            let portmap = if self.node_attr.inverted().unwrap() {
                self.output_port_map()
            } else {
                self.input_port_map()
            };
            let assigned_ports = portmap.iter().filter(|p| p.1 .0 == idx);
            let mut incoming = LightResult::default();
            for port in assigned_ports {
                if let Some(input_data) = incoming_data.get(port.0) {
                    incoming.insert(port.1 .1.clone(), input_data.clone());
                }
            }
            incoming
        } else {
            self.incoming_edges(idx)
        }
    }
    fn is_stale_node(&self, idx: NodeIndex) -> bool {
        let neighbors = self.g.0.neighbors_undirected(idx);
        neighbors.count() == 0 && !self.input_port_map().iter().any(|p| p.1 .0 == idx)
    }
    fn analyze_group(
        &mut self,
        incoming_data: &LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let is_inverted = self.node_attr.inverted()?;
        if is_inverted {
            self.invert_graph()?;
        }
        let g_clone = self.g.0.clone();
        let group_name = self.name();
        if !&self.g.is_single_tree() {
            warn!(
                "Group {group_name} contains unconnected sub-trees. Analysis might not be complete."
            );
        }
        let sorted = toposort(&self.g.0, None)
            .map_err(|_| OpossumError::Analysis("topological sort failed".into()))?;
        let mut light_result = LightResult::default();
        for idx in sorted {
            let node = g_clone.node_weight(idx).unwrap();
            if self.is_stale_node(idx) {
                let node_name = node.optical_ref.borrow().name();
                warn!("Group {group_name} contains stale (completely unconnected) node {node_name}. Skipping.");
            } else {
                // Check if node is group src node
                let incoming_edges = self.get_incoming(idx, incoming_data);
                let outgoing_edges: LightResult = node
                    .optical_ref
                    .borrow_mut()
                    .analyze(incoming_edges, analyzer_type)?;
                // Check if node is group sink node
                if self.is_group_sink_node(idx) {
                    let portmap = if is_inverted {
                        self.input_port_map()
                    } else {
                        self.output_port_map()
                    };
                    let assigned_ports = portmap.iter().filter(|p| p.1 .0 == idx);
                    for port in assigned_ports {
                        if let Some(light_data) = outgoing_edges.get(&port.1 .1) {
                            light_result.insert(port.0.clone(), light_data.clone());
                        }
                    }
                } else {
                    for outgoing_edge in outgoing_edges {
                        self.set_outgoing_edge_data(idx, &outgoing_edge.0, Some(outgoing_edge.1));
                    }
                }
            }
        }
        if is_inverted {
            self.invert_graph()?;
        } // revert initial inversion (if necessary)
        Ok(light_result)
    }

    /// Sets the expansion flag of this [`NodeGroup`].  
    /// If true, the group expands and the internal nodes of this group are displayed in the dot format.
    /// If false, only the group node itself is displayed and the internal setup is not shown
    ///
    /// # Errors
    /// This function returns an error if the property "expand view" does not exist and the
    /// function [`get_bool()`](../properties/struct.Properties.html#method.get_bool) fails
    pub fn shall_expand(&self) -> OpmResult<bool> {
        self.node_attr.get_property_bool("expand view")
    }

    /// Defines and returns the node/port identifier to connect the edges in the dot format
    /// # Arguments
    /// * `port_name`:            name of the external port of the group
    /// * `parent_identifier`:    String that contains the hierarchical structure: `parentidx_childidx_childofchildidx` ...
    ///
    /// # Errors
    /// Throws an [`OpossumError::OpticGroup`] if the specified port name is not mapped as input or output
    ///
    /// # Panics
    /// This funciton panics if the specified `port_name` is not mapped to a port
    pub fn get_mapped_port_str(
        &self,
        port_name: &str,
        parent_identifier: &str,
    ) -> OpmResult<String> {
        if self.shall_expand()? {
            if self.input_port_map().contains_key(port_name) {
                let input_port_map = self.input_port_map();
                let port = input_port_map.get(port_name).unwrap();

                Ok(format!(
                    "{}_i{}:{}",
                    parent_identifier,
                    port.0.index(),
                    port.1
                ))
            } else if self.output_port_map().contains_key(port_name) {
                let output_port_map = self.output_port_map();
                let port = output_port_map.get(port_name).unwrap();

                Ok(format!(
                    "{}_i{}:{}",
                    parent_identifier,
                    port.0.index(),
                    port.1
                ))
            } else {
                Err(OpossumError::OpticGroup(format!(
                    "port {port_name} is not mapped"
                )))
            }
        } else {
            Ok(format!("{parent_identifier}:{port_name}"))
        }
    }

    /// Define if a [`NodeGroup`] should be displayed expanded or not in diagram.
    ///
    /// # Errors
    /// This function returns an error if the property "expand view" can not be set
    pub fn expand_view(&mut self, expand_view: bool) -> OpmResult<()> {
        self.node_attr
            .set_property("expand view", expand_view.into())
    }
    /// Creates the dot-format string which describes the edge that connects two nodes
    /// parameters:
    /// * `end_node_idx`:         [`NodeIndex`] of the node that should be connected
    /// * `light_port`:           port name that should be connected
    /// * `parent_identifier`:    String that contains the hierarchical structure: `parentidx_childidx_childofchildidx` ...
    ///
    /// Returns the result of the edge strnig for the dot format
    fn create_node_edge_str(
        &self,
        end_node_idx: NodeIndex,
        light_port: &str,
        mut parent_identifier: String,
    ) -> OpmResult<String> {
        let node = self
            .g
            .0
            .node_weight(end_node_idx)
            .unwrap()
            .optical_ref
            .borrow();

        parent_identifier = if parent_identifier.is_empty() {
            format!("i{}", end_node_idx.index())
        } else {
            format!("{}_i{}", &parent_identifier, end_node_idx.index())
        };

        if node.node_type() == "group" {
            let group_node: &Self = node.as_group()?;
            Ok(group_node.get_mapped_port_str(light_port, &parent_identifier)?)
        } else {
            Ok(format!("{parent_identifier}:{light_port}"))
        }
    }
    /// Creates the dot format of the [`NodeGroup`] in its expanded view
    /// # Attributes:
    /// * `node_index`:           [`NodeIndex`] of the group
    /// * `name`:                 name of the node
    /// * `inverted`:             boolean that descries wether the node is inverted or not
    /// * `parent_identifier`:    String that contains the hierarchical structure: `parentidx_childidx_childofchildidx` ...
    ///
    /// Returns the result of the dot string that describes this node
    fn to_dot_expanded_view(
        &self,
        node_index: &str,
        name: &str,
        inverted: bool,
        mut parent_identifier: String,
        rankdir: &str,
    ) -> OpmResult<String> {
        let inv_string = if inverted { "(inv)" } else { "" };
        parent_identifier = if parent_identifier.is_empty() {
            format!("i{node_index}")
        } else {
            format!("{}_i{}", &parent_identifier, node_index)
        };
        let mut dot_string = format!(
            "  subgraph {parent_identifier} {{\n\tlabel=\"{name}{inv_string}\"\n\tfontsize=8\n\tcluster=true\n\t"
        );

        for node_idx in self.g.0.node_indices() {
            let node = self.g.0.node_weight(node_idx).unwrap();
            dot_string += &node.optical_ref.borrow().to_dot(
                &format!("{}", node_idx.index()),
                &node.optical_ref.borrow().name(),
                node.optical_ref.borrow().properties().inverted()?,
                &node.optical_ref.borrow().ports(),
                parent_identifier.clone(),
                rankdir,
            )?;
        }
        for edge in self.g.0.edge_indices() {
            let light: &Light = self.g.0.edge_weight(edge).unwrap();
            let end_nodes = self.g.0.edge_endpoints(edge).unwrap();

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

            dot_string.push_str(&format!("  {src_edge_str} -> {target_edge_str} \n"));
        }
        dot_string += "}";
        Ok(dot_string)
    }

    /// Creates the dot format of the [`NodeGroup`] in its collapsed view
    ///
    /// # Attributes:            [`NodeIndex`] of the group
    /// * `name`:                 name of the node
    /// * `inverted`:             boolean that descries wether the node is inverted or not
    /// * `ports`:               
    /// * `parent_identifier`:    String that contains the hierarchical structure: `parentidx_childidx_childofchildidx` ...
    ///
    /// Returns the result of the dot string that describes this node
    fn to_dot_collapsed_view(
        &self,
        node_index: &str,
        name: &str,
        inverted: bool,
        ports: &OpticPorts,
        mut parent_identifier: String,
        rankdir: &str,
    ) -> String {
        let inv_string = if inverted { " (inv)" } else { "" };
        let node_name = format!("{name}{inv_string}");
        parent_identifier = if parent_identifier.is_empty() {
            format!("i{node_index}")
        } else {
            format!("{}_i{node_index}", &parent_identifier)
        };
        let mut dot_str = format!("\t{parent_identifier} [\n\t\tshape=plaintext\n");
        let mut indent_level = 2;
        dot_str.push_str(&self.add_html_like_labels(&node_name, &mut indent_level, ports, rankdir));
        dot_str
    }
    fn invert_graph(&mut self) -> OpmResult<()> {
        for node in self.g.0.node_weights_mut() {
            node.optical_ref
                .borrow_mut()
                .set_property("inverted", true.into())
                .map_err(|_| {
                    OpossumError::OpticGroup(
                        "group cannot be inverted because it contains a non-invertable node".into(),
                    )
                })?;
        }
        for edge in self.g.0.edge_weights_mut() {
            edge.inverse();
        }
        self.g.0.reverse();
        Ok(())
    }
}

impl Optical for NodeGroup {
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        let Proptype::OpticPorts(ports_to_be_set) = self.properties().get("apertures").unwrap()
        else {
            panic!("failed to set global input and exit apertures");
        };
        for p in &self.input_port_map() {
            ports.create_input(p.0).unwrap();
        }
        for p in &self.output_port_map() {
            ports.create_output(p.0).unwrap();
        }
        if self.properties().inverted().unwrap() {
            ports.set_inverted(true);
        }
        ports.set_apertures(ports_to_be_set.clone()).unwrap();
        ports
    }
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        self.analyze_group(&incoming_data, analyzer_type)
    }
    fn as_group(&self) -> OpmResult<&NodeGroup> {
        Ok(self)
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.node_attr.set_property(name, prop)
    }
    fn after_deserialization_hook(&mut self) -> OpmResult<()> {
        // Synchronize properties with (internal) graph structure.
        if let Proptype::OpticGraph(g) = &self.node_attr.get_property("graph")? {
            self.g = g.clone();
        }
        Ok(())
    }
    fn report(&self) -> Option<NodeReport> {
        let mut group_props = Properties::default();
        for node_idx in self.g.0.node_indices() {
            let node = self.g.0.node_weight(node_idx).unwrap().optical_ref.borrow();
            if let Some(node_report) = node.report() {
                if !(group_props.contains(&node.name())) {
                    group_props
                        .create(&node.name(), "", None, node_report.into())
                        .unwrap();
                }
            }
        }
        Some(NodeReport::new(
            &self.node_type(),
            &self.name(),
            group_props,
        ))
    }
    fn is_detector(&self) -> bool {
        self.g.contains_detector()
    }

    fn export_data(&self, report_dir: &Path) -> OpmResult<Option<image::RgbImage>> {
        let detector_nodes = self
            .g
            .0
            .node_weights()
            .filter(|node| node.optical_ref.borrow().is_detector());
        for node in detector_nodes {
            node.optical_ref.borrow().export_data(report_dir)?;
        }
        Ok(None)
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
}

impl Dottable for NodeGroup {
    fn to_dot(
        &self,
        node_index: &str,
        name: &str,
        inverted: bool,
        ports: &OpticPorts,
        parent_identifier: String,
        rankdir: &str,
    ) -> OpmResult<String> {
        let mut cloned_self = self.clone();
        if self.node_attr.inverted()? {
            cloned_self.invert_graph()?;
        }
        if self.shall_expand()? {
            cloned_self.to_dot_expanded_view(node_index, name, inverted, parent_identifier, rankdir)
        } else {
            Ok(cloned_self.to_dot_collapsed_view(
                node_index,
                name,
                inverted,
                ports,
                parent_identifier,
                rankdir,
            ))
        }
    }
    fn node_color(&self) -> &str {
        "yellow"
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        joule,
        lightdata::DataEnergy,
        millimeter, nanometer,
        nodes::test_helper::test_helper::*,
        nodes::{BeamSplitter, Detector, Dummy, Source},
        optical::Optical,
        position_distributions::Hexapolar,
        ray::SplittingConfig,
        rays::Rays,
        spectrum_helper::create_he_ne_spec,
    };
    use approx::assert_abs_diff_eq;
    use log::Level;
    #[test]
    fn default() {
        let node = NodeGroup::default();
        assert_eq!(node.g.0.node_count(), 0);
        assert_eq!(node.g.0.edge_count(), 0);
        assert!(node.input_port_map().is_empty());
        assert!(node.output_port_map().is_empty());
        assert_eq!(node.name(), "group");
        assert_eq!(node.node_type(), "group");
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "yellow");
        assert!(node.as_group().is_ok());
    }
    #[test]
    fn new() {
        let node = NodeGroup::new("test");
        assert_eq!(node.name(), "test");
    }
    #[test]
    fn is_detector() {
        let mut node = NodeGroup::default();
        assert_eq!(node.is_detector(), false);
        node.add_node(Detector::default()).unwrap();
        assert_eq!(node.is_detector(), true);
    }
    #[test]
    fn add_node() {
        let mut og = NodeGroup::default();
        og.add_node(Dummy::new("n1")).unwrap();
        assert_eq!(og.g.0.node_count(), 1);
    }
    #[test]
    fn add_node_inverted() {
        let mut og = NodeGroup::default();
        og.set_property("inverted", true.into()).unwrap();
        assert!(og.add_node(Dummy::new("n1")).is_err());
    }
    #[test]
    fn inverted() {
        test_inverted::<NodeGroup>()
    }
    #[test]
    fn connect_nodes() {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(Dummy::new("n1")).unwrap();
        let sn2_i = og.add_node(Dummy::new("n2")).unwrap();
        // wrong port names
        assert!(og.connect_nodes(sn1_i, "wrong", sn2_i, "front").is_err());
        assert_eq!(og.g.0.edge_count(), 0);
        assert!(og.connect_nodes(sn1_i, "rear", sn2_i, "wrong").is_err());
        assert_eq!(og.g.0.edge_count(), 0);
        // wrong node index
        assert!(og.connect_nodes(5.into(), "rear", sn2_i, "front").is_err());
        assert_eq!(og.g.0.edge_count(), 0);
        assert!(og.connect_nodes(sn1_i, "rear", 5.into(), "front").is_err());
        assert_eq!(og.g.0.edge_count(), 0);
        // correct usage
        assert!(og.connect_nodes(sn1_i, "rear", sn2_i, "front").is_ok());
        assert_eq!(og.g.0.edge_count(), 1);
    }
    #[test]
    fn connect_nodes_inverted() {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(Dummy::new("n1")).unwrap();
        let sn2_i = og.add_node(Dummy::new("n2")).unwrap();
        og.set_property("inverted", true.into()).unwrap();
        assert!(og.connect_nodes(sn1_i, "rear", sn2_i, "front").is_err());
    }
    #[test]
    fn connect_nodes_update_port_mapping() {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(Dummy::new("n1")).unwrap();
        let sn2_i = og.add_node(Dummy::new("n2")).unwrap();

        og.map_input_port(sn2_i, "front", "input").unwrap();
        og.map_output_port(sn1_i, "rear", "output").unwrap();
        assert_eq!(og.input_port_map().len(), 1);
        assert_eq!(og.output_port_map().len(), 1);
        og.connect_nodes(sn1_i, "rear", sn2_i, "front").unwrap();
        // delete no longer valid port mapping
        assert_eq!(og.input_port_map().len(), 0);
        assert_eq!(og.output_port_map().len(), 0);
    }
    #[test]
    fn input_nodes() {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(Dummy::new("n1")).unwrap();
        let sn2_i = og.add_node(Dummy::new("n2")).unwrap();
        let sub_node3 = BeamSplitter::new("test", &SplittingConfig::Ratio(0.5)).unwrap();
        let sn3_i = og.add_node(sub_node3).unwrap();
        og.connect_nodes(sn1_i, "rear", sn2_i, "front").unwrap();
        og.connect_nodes(sn2_i, "rear", sn3_i, "input1").unwrap();
        assert_eq!(og.input_nodes(), vec![0.into(), 2.into()])
    }
    #[test]
    fn output_nodes() {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(Dummy::new("n1")).unwrap();
        let sub_node1 = BeamSplitter::new("test", &SplittingConfig::Ratio(0.5)).unwrap();
        let sn2_i = og.add_node(sub_node1).unwrap();
        let sn3_i = og.add_node(Dummy::new("n3")).unwrap();
        og.connect_nodes(sn1_i, "rear", sn2_i, "input1").unwrap();
        og.connect_nodes(sn2_i, "out1_trans1_refl2", sn3_i, "front")
            .unwrap();
        assert_eq!(og.input_nodes(), vec![0.into(), 1.into()])
    }
    #[test]
    fn map_input_port() {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(Dummy::new("n1")).unwrap();
        let sn2_i = og.add_node(Dummy::new("n2")).unwrap();
        og.connect_nodes(sn1_i, "rear", sn2_i, "front").unwrap();

        // wrong port name
        assert!(og.map_input_port(sn1_i, "wrong", "input").is_err());
        assert!(og.input_port_map().is_empty());
        // wrong node index
        assert!(og.map_input_port(5.into(), "front", "input").is_err());
        assert!(og.input_port_map().is_empty());
        // map output port
        assert!(og.map_input_port(sn2_i, "rear", "input").is_err());
        assert!(og.input_port_map().is_empty());
        // map internal node
        assert!(og.map_input_port(sn2_i, "front", "input").is_err());
        assert!(og.input_port_map().is_empty());
        // correct usage
        assert!(og.map_input_port(sn1_i, "front", "input").is_ok());
        assert_eq!(og.input_port_map().len(), 1);
    }
    #[test]
    fn map_input_port_half_connected_nodes() {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(Dummy::new("n1")).unwrap();
        let sn2_i = og.add_node(BeamSplitter::default()).unwrap();
        og.connect_nodes(sn1_i, "rear", sn2_i, "input1").unwrap();

        // node port already internally connected
        assert!(og.map_input_port(sn2_i, "input1", "bs_input").is_err());

        // correct usage
        assert!(og.map_input_port(sn1_i, "front", "input").is_ok());
        assert!(og.map_input_port(sn2_i, "input2", "bs_input").is_ok());
        assert_eq!(og.input_port_map().len(), 2);
    }
    #[test]
    fn map_output_port() {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(Dummy::new("n1")).unwrap();
        let sn2_i = og.add_node(Dummy::new("n2")).unwrap();
        og.connect_nodes(sn1_i, "rear", sn2_i, "front").unwrap();

        // wrong port name
        assert!(og.map_output_port(sn2_i, "wrong", "output").is_err());
        assert!(og.output_port_map().is_empty());
        // wrong node index
        assert!(og.map_output_port(5.into(), "rear", "output").is_err());
        assert!(og.output_port_map().is_empty());
        // map input port
        assert!(og.map_output_port(sn1_i, "front", "output").is_err());
        assert!(og.output_port_map().is_empty());
        // map internal node
        assert!(og.map_output_port(sn1_i, "rear", "output").is_err());
        assert!(og.output_port_map().is_empty());
        // correct usage
        assert!(og.map_output_port(sn2_i, "rear", "output").is_ok());
        assert_eq!(og.output_port_map().len(), 1);
    }
    #[test]
    fn map_output_port_half_connected_nodes() {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(BeamSplitter::default()).unwrap();
        let sn2_i = og.add_node(Dummy::new("n2")).unwrap();
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
        assert_eq!(og.output_port_map().len(), 2);
    }
    #[test]
    fn ports() {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(Dummy::new("n1")).unwrap();
        let sn2_i = og.add_node(Dummy::new("n2")).unwrap();
        og.connect_nodes(sn1_i, "rear", sn2_i, "front").unwrap();
        assert!(og.ports().input_names().is_empty());
        assert!(og.ports().output_names().is_empty());
        og.map_input_port(sn1_i, "front", "input").unwrap();
        assert!(og.ports().input_names().contains(&("input".to_string())));
        og.map_output_port(sn2_i, "rear", "output").unwrap();
        assert!(og.ports().output_names().contains(&("output".to_string())));
    }
    #[test]
    fn ports_inverted() {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(Dummy::new("n1")).unwrap();
        let sn2_i = og.add_node(Dummy::new("n2")).unwrap();
        og.connect_nodes(sn1_i, "rear", sn2_i, "front").unwrap();
        og.map_input_port(sn1_i, "front", "input").unwrap();
        og.map_output_port(sn2_i, "rear", "output").unwrap();
        og.set_property("inverted", true.into()).unwrap();
        assert!(og.ports().output_names().contains(&("input".to_string())));
        assert!(og.ports().input_names().contains(&("output".to_string())));
    }
    fn prepare_group() -> NodeGroup {
        let mut group = NodeGroup::default();
        let g1_n1 = group.add_node(Dummy::new("node1")).unwrap();
        let g1_n2 = group
            .add_node(BeamSplitter::new("test", &SplittingConfig::Ratio(0.6)).unwrap())
            .unwrap();
        group
            .map_output_port(g1_n2, "out1_trans1_refl2", "output")
            .unwrap();
        group.map_input_port(g1_n1, "front", "input").unwrap();
        group.connect_nodes(g1_n1, "rear", g1_n2, "input1").unwrap();
        group
    }
    #[test]
    fn analyze_empty() {
        let mut node = NodeGroup::default();
        let output = node
            .analyze(LightResult::default(), &AnalyzerType::Energy)
            .unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_wrong_input_data() {
        let mut group = prepare_group();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("wrong".into(), input_light.clone());
        let output = group.analyze(input, &AnalyzerType::Energy).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze() {
        let mut group = prepare_group();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("input".into(), input_light.clone());
        let output = group.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("output"));
        let output = output.get("output").unwrap().clone();
        let energy = if let LightData::Energy(data) = output {
            data.spectrum.total_energy()
        } else {
            0.0
        };
        assert_abs_diff_eq!(energy, 0.6, epsilon = f64::EPSILON);
    }
    #[test]
    fn analyze_empty_group() {
        let mut group = NodeGroup::default();
        let input = LightResult::default();
        let output = group.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_subtree_warning() {
        testing_logger::setup();
        let mut group = NodeGroup::default();
        let d1 = group.add_node(Dummy::default()).unwrap();
        let d2 = group.add_node(Dummy::default()).unwrap();
        let d3 = group.add_node(Dummy::default()).unwrap();
        let d4 = group.add_node(Dummy::default()).unwrap();
        group.connect_nodes(d1, "rear", d2, "front").unwrap();
        group.connect_nodes(d3, "rear", d4, "front").unwrap();
        group.map_input_port(d1, "front", "input").unwrap();
        let input = LightResult::default();
        let output = group.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 1);
            assert_eq!(
                captured_logs[0].body,
                "Group group contains unconnected sub-trees. Analysis might not be complete."
            );
            assert_eq!(captured_logs[0].level, Level::Warn);
        });
    }
    #[test]
    fn analyze_stale_node() {
        testing_logger::setup();
        let mut group = NodeGroup::default();
        let d1 = group.add_node(Dummy::default()).unwrap();
        let _ = group.add_node(Dummy::new("stale node")).unwrap();
        group.map_input_port(d1, "front", "input").unwrap();
        let mut input = LightResult::default();
        input.insert(
            "input".into(),
            LightData::Geometric(
                Rays::new_uniform_collimated(
                    nanometer!(1054.0),
                    joule!(1.0),
                    &Hexapolar::new(millimeter!(1.0), 1).unwrap(),
                )
                .unwrap(),
            ),
        );
        let output = group.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 2);
            assert_eq!(
                captured_logs[0].body,
                "Group group contains unconnected sub-trees. Analysis might not be complete."
            );
            assert_eq!(
                captured_logs[1].body,
                "Group group contains stale (completely unconnected) node stale node. Skipping."
            );

            assert_eq!(captured_logs[0].level, Level::Warn);
        });
    }
    #[test]
    fn analyze_inverse() {
        let mut group = prepare_group();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        group.set_property("inverted", true.into()).unwrap();
        input.insert("output".into(), input_light);
        let output = group.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("input"));
        let output = output.get("input").unwrap().clone();
        let energy = if let LightData::Energy(data) = output {
            data.spectrum.total_energy()
        } else {
            0.0
        };
        assert_abs_diff_eq!(energy, 0.6, epsilon = f64::EPSILON);
    }
    #[test]
    fn analyze_inverse_with_src() {
        let mut group = NodeGroup::default();
        let g1_n1 = group.add_node(Source::default()).unwrap();
        let g1_n2 = group.add_node(Dummy::new("node1")).unwrap();
        group.map_output_port(g1_n2, "rear", "output").unwrap();
        group.connect_nodes(g1_n1, "out1", g1_n2, "front").unwrap();
        group.set_property("inverted", true.into()).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("output".into(), input_light);
        let output = group.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_err());
    }
    #[test]
    fn report_default() {
        let group = NodeGroup::default();
        assert!(group.report().is_some());
        let report = group.report().unwrap();
        let nr_of_props = report.properties().iter().fold(0, |s: usize, _p| s + 1);
        assert_eq!(nr_of_props, 0);
    }
}
