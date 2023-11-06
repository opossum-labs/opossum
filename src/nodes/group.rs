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
use crate::{optic_ports::OpticPorts, optical::Optical};
use petgraph::prelude::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::{algo::*, Direction};
use serde_derive::Serialize;
use std::collections::HashMap;

/// Mappin of group internal ports to externally visible ports.
pub type PortMap = HashMap<String, (NodeIndex, String)>;
impl From<PortMap> for Proptype {
    fn from(value: PortMap) -> Self {
        Proptype::GroupPortMap(value)
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
pub struct NodeGroup {
    #[serde(skip)]
    g: OpticGraph,
    props: Properties,
}

fn create_default_props() -> Properties {
    let mut props = Properties::new("group", "group");
    props
        .create(
            "expand view",
            "show group fully expanded in dot diagram?",
            None,
            false.into(),
        )
        .unwrap();
    props
        .create("graph", "optical graph", None, OpticGraph::default().into())
        .unwrap();
    props
        .create(
            "input port map",
            "mapping of internal input ports to external ones",
            None,
            PortMap::new().into(),
        )
        .unwrap();
    props
        .create(
            "output port map",
            "mapping of internal output ports to external ones",
            None,
            PortMap::new().into(),
        )
        .unwrap();
    props
}

impl Default for NodeGroup {
    fn default() -> Self {
        Self {
            g: Default::default(),
            props: create_default_props(),
        }
    }
}
impl NodeGroup {
    /// Creates a new [`NodeGroup`].
    pub fn new(name: &str) -> Self {
        let mut props = create_default_props();
        props.set("name", name.into()).unwrap();
        Self {
            props,
            ..Default::default()
        }
    }
    /// Add a given [`Optical`] to the (sub-)graph of this [`NodeGroup`].
    ///
    /// This command just adds an [`Optical`] but does not connect it to existing nodes in the (sub-)graph. The given node is
    /// consumed (owned) by the [`NodeGroup`].
    ///
    /// ## Errors
    ///
    /// An error is returned if the [`NodeGroup`] is set as inverted (which would lead to strange behaviour).
    pub fn add_node<T: Optical + 'static>(&mut self, node: T) -> OpmResult<NodeIndex> {
        if self.properties().inverted() {
            return Err(OpossumError::OpticGroup(
                "cannot add nodes if group is set as inverted".into(),
            ));
        }
        let idx = self.g.add_node(node);
        self.props
            .set_unchecked("graph", self.g.clone().into())
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
    /// ## Errors
    ///
    /// This function returns an [`OpossumError`] if
    ///   - the group is set as `inverted`. Connectiing subnodes of an inverted group node would result in strange behaviour.
    ///   - the source node / port or target node / port does not exist.
    ///   - the source node / port or target node / port is already connected.
    ///   - the node connection would form a loop in the graph.
    pub fn connect_nodes(
        &mut self,
        src_node: NodeIndex,
        src_port: &str,
        target_node: NodeIndex,
        target_port: &str,
    ) -> OpmResult<()> {
        if self.properties().inverted() {
            return Err(OpossumError::OpticGroup(
                "cannot connect nodes if group is set as inverted".into(),
            ));
        }
        self.g
            .connect_nodes(src_node, src_port, target_node, target_port)?;
        self.props
            .set_unchecked("graph", self.g.clone().into())
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
        let input_port_map = self.props.get("input port map").unwrap().clone();
        if let Proptype::GroupPortMap(input_port_map) = input_port_map {
            input_port_map
        } else {
            panic!("wrong data type")
        }
    }
    fn set_input_port_map(&mut self, port_map: PortMap) {
        self.props
            .set_unchecked("input port map", port_map.into())
            .unwrap();
    }
    fn output_port_map(&self) -> PortMap {
        let output_port_map = self.props.get("output port map").unwrap().clone();
        if let Proptype::GroupPortMap(output_port_map) = output_port_map {
            output_port_map
        } else {
            panic!("wrong data type")
        }
    }
    fn set_output_port_map(&mut self, port_map: PortMap) {
        self.props
            .set_unchecked("output port map", port_map.into())
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
            .ok_or(OpossumError::OpticGroup(
                "internal node index not found".into(),
            ))?;
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
            .ok_or(OpossumError::OpticGroup(
                "internal node index not found".into(),
            ))?;
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
            .map(|e| {
                (
                    e.weight().target_port().to_owned(),
                    e.weight().data().cloned(),
                )
            })
            .collect::<HashMap<String, Option<LightData>>>()
    }
    fn set_outgoing_edge_data(&mut self, idx: NodeIndex, port: String, data: Option<LightData>) {
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
    fn analyze_group(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let is_inverted = self.props.get_bool("inverted").unwrap().unwrap();
        if is_inverted {
            self.invert_graph()?;
        }
        let g_clone = self.g.0.clone();
        let mut group_srcs = g_clone.externals(Direction::Incoming);
        let mut light_result = LightResult::default();
        let sorted = toposort(&self.g.0, None)
            .map_err(|_| OpossumError::Analysis("topological sort failed".into()))?;
        for idx in sorted {
            // Check if node is group src node
            let incoming_edges = if group_srcs.any(|gs| gs == idx) {
                // get from incoming_data
                let portmap = if is_inverted {
                    self.output_port_map()
                } else {
                    self.input_port_map()
                };
                let assigned_ports = portmap.iter().filter(|p| p.1 .0 == idx);
                let mut incoming = LightResult::default();
                for port in assigned_ports {
                    let input_data = incoming_data.get(port.0).unwrap_or(&None);
                    incoming.insert(port.1 .1.to_owned(), input_data.clone());
                }
                incoming
            } else {
                self.incoming_edges(idx)
            };
            let node = g_clone.node_weight(idx).unwrap();
            let outgoing_edges: HashMap<String, Option<LightData>> = node
                .optical_ref
                .borrow_mut()
                .analyze(incoming_edges, analyzer_type)?;
            let mut group_sinks = g_clone.externals(Direction::Outgoing);
            // Check if node is group sink node
            if group_sinks.any(|gs| gs == idx) {
                let portmap = if is_inverted {
                    self.input_port_map()
                } else {
                    self.output_port_map()
                };
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
        if is_inverted {
            self.invert_graph()?;
        } // revert initial inversion (if necessary)
        Ok(light_result)
    }

    /// Sets the expansion flag of this [`NodeGroup`].  
    /// If true, the group expands and the internal nodes of this group are displayed in the dot format.
    /// If false, only the group node itself is displayed and the internal setup is not shown
    pub fn shall_expand(&self) -> bool {
        self.props.get_bool("expand view").unwrap().unwrap()
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
    ) -> OpmResult<String> {
        if self.shall_expand() {
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
        self.props.set("expand view", expand_view.into()).unwrap();
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

        if node.properties().node_type()? == "group" {
            let group_node: &NodeGroup = node.as_group()?;
            Ok(group_node.get_mapped_port_str(light_port, parent_identifier)?)
        } else {
            Ok(format!("{}:{}", parent_identifier, light_port))
        }
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
        rankdir: &str,
    ) -> OpmResult<String> {
        let inv_string = if inverted { "(inv)" } else { "" };
        parent_identifier = if parent_identifier.is_empty() {
            format!("i{}", node_index)
        } else {
            format!("{}_i{}", &parent_identifier, node_index)
        };
        let mut dot_string = format!(
            "  subgraph {} {{\n\tlabel=\"{}{}\"\n\tfontsize=15\n\tcluster=true\n\t",
            parent_identifier, name, inv_string
        );

        for node_idx in self.g.0.node_indices() {
            let node = self.g.0.node_weight(node_idx).unwrap();
            dot_string += &node.optical_ref.borrow().to_dot(
                &format!("{}", node_idx.index()),
                node.optical_ref.borrow().properties().name()?,
                node.optical_ref.borrow().properties().inverted(),
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

            dot_string.push_str(&format!("  {} -> {} \n", src_edge_str, target_edge_str));
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
        rankdir: &str,
    ) -> OpmResult<String> {
        let inv_string = if inverted { " (inv)" } else { "" };
        let node_name = format!("{}{}", name, inv_string);
        parent_identifier = if parent_identifier.is_empty() {
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
            rankdir,
        ));
        Ok(dot_str)
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
        for p in self.input_port_map().iter() {
            ports.add_input(p.0).unwrap();
        }
        for p in self.output_port_map().iter() {
            ports.add_output(p.0).unwrap();
        }
        if self.properties().inverted() {
            ports.set_inverted(true);
        }
        ports
    }
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        self.analyze_group(incoming_data, analyzer_type)
    }
    fn as_group(&self) -> OpmResult<&NodeGroup> {
        Ok(self)
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.props.set(name, prop)
    }
    fn after_deserialization_hook(&mut self) -> OpmResult<()> {
        // Synchronize properties with (internal) graph structure.
        if let Proptype::OpticGraph(g) = &self.props.get("graph")? {
            self.g = g.clone();
        }
        Ok(())
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
        if self.props.get_bool("inverted").unwrap().unwrap() {
            cloned_self.invert_graph()?;
        }
        if self.shall_expand() {
            cloned_self.to_dot_expanded_view(node_index, name, inverted, parent_identifier, rankdir)
        } else {
            cloned_self.to_dot_collapsed_view(
                node_index,
                name,
                inverted,
                ports,
                parent_identifier,
                rankdir,
            )
        }
    }
    fn node_color(&self) -> &str {
        "yellow"
    }
}

#[cfg(test)]
mod test {
    use approx::assert_abs_diff_eq;

    use super::*;
    use crate::{
        lightdata::DataEnergy,
        nodes::{BeamSplitter, Dummy, Source},
        optical::Optical,
        spectrum::create_he_ne_spectrum,
    };
    #[test]
    fn default() {
        let node = NodeGroup::default();
        assert_eq!(node.g.0.node_count(), 0);
        assert_eq!(node.g.0.edge_count(), 0);
        assert!(node.input_port_map().is_empty());
        assert!(node.output_port_map().is_empty());
        assert_eq!(node.properties().name().unwrap(), "group");
        assert_eq!(node.properties().node_type().unwrap(), "group");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted(), false);
        assert_eq!(node.node_color(), "yellow");
        assert!(node.as_group().is_ok());
    }
    #[test]
    fn new() {
        let node = NodeGroup::new("test");
        assert_eq!(node.properties().name().unwrap(), "test");
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
        let mut og = NodeGroup::default();
        og.set_property("inverted", true.into()).unwrap();
        assert_eq!(og.properties().inverted(), true);
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
        let sub_node3 = BeamSplitter::new("test", 0.5).unwrap();
        let sn3_i = og.add_node(sub_node3).unwrap();
        og.connect_nodes(sn1_i, "rear", sn2_i, "front").unwrap();
        og.connect_nodes(sn2_i, "rear", sn3_i, "input1").unwrap();
        assert_eq!(og.input_nodes(), vec![0.into(), 2.into()])
    }
    #[test]
    fn output_nodes() {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(Dummy::new("n1")).unwrap();
        let sub_node1 = BeamSplitter::new("test", 0.5).unwrap();
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
            .add_node(BeamSplitter::new("test", 0.6).unwrap())
            .unwrap();
        group
            .map_output_port(g1_n2, "out1_trans1_refl2", "output")
            .unwrap();
        group.map_input_port(g1_n1, "front", "input").unwrap();
        group.connect_nodes(g1_n1, "rear", g1_n2, "input1").unwrap();
        group
    }
    #[test]
    fn analyze() {
        let mut group = prepare_group();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        });
        input.insert("input".into(), Some(input_light.clone()));
        let output = group.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("output"));
        let output = output.get("output").unwrap().clone().unwrap();
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
    fn analyze_wrong_input_data() {
        let mut group = prepare_group();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        });
        input.insert("wrong".into(), Some(input_light.clone()));
        let output = group.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("output"));
        let output = output.get("output").unwrap().clone();
        assert!(output.is_none());
    }
    #[test]
    fn analyze_inverse() {
        let mut group = prepare_group();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        });
        group.set_property("inverted", true.into()).unwrap();
        input.insert("output".into(), Some(input_light.clone()));
        let output = group.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("input"));
        let output = output.get("input").unwrap().clone().unwrap();
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
            spectrum: create_he_ne_spectrum(1.0),
        });
        input.insert("output".into(), Some(input_light.clone()));
        let output = group.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_err());
    }
}
