#![warn(missing_docs)]
use super::node_attr::NodeAttr;
use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    optic_graph::OpticGraph,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Proptype},
    reporter::NodeReport,
    utils::geom_transformation::Isometry,
    SceneryResources,
};
use petgraph::prelude::NodeIndex;
use std::{cell::RefCell, collections::BTreeMap, path::Path, rc::Rc};
use uom::si::f64::Length;

#[derive(Debug, Clone)]
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
    g: OpticGraph,
    node_attr: NodeAttr,
    input_port_distances: BTreeMap<String, Length>,
}
impl Default for NodeGroup {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("group");
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
        Self {
            g: OpticGraph::default(),
            input_port_distances: BTreeMap::default(),
            node_attr,
        }
    }
}
impl NodeGroup {
    /// Creates a new [`NodeGroup`].
    /// # Attributes
    /// * `name`: name of the  [`NodeGroup`]
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut group = Self::default();
        group.node_attr.set_name(name);
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
    /// This function panics if the property "graph" can not be updated. Produces an error of type [`OpossumError::Properties`]
    pub fn add_node<T: Optical + 'static>(&mut self, node: T) -> OpmResult<NodeIndex> {
        let idx = self.g.add_node(node);
        self.node_attr
            .set_property("graph", self.g.clone().into())
            .unwrap();
        idx
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
        distance: Length,
    ) -> OpmResult<()> {
        self.g
            .connect_nodes(src_node, src_port, target_node, target_port, distance)?;
        self.node_attr
            .set_property("graph", self.g.clone().into())
            .unwrap();
        Ok(())
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
        self.g
            .map_input_port(input_node, internal_name, external_name)?;
        self.node_attr.set_property("graph", self.g.clone().into())
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
        self.g
            .map_output_port(output_node, internal_name, external_name)?;
        self.node_attr.set_property("graph", self.g.clone().into())
    }
    /// Defines and returns the node/port identifier to connect the edges in the dot format
    /// # Parameters
    /// * `port_name`:            name of the external port of the group
    /// * `node_id`:    String containing the uuid of the parent node
    ///
    /// # Errors
    /// Throws an [`OpossumError::OpticGroup`] if the specified port name is not mapped as input or output
    ///
    /// # Panics
    /// This function panics if the specified `port_name` is not mapped to a port
    pub fn get_mapped_port_str(&self, port_name: &str, node_id: &str) -> OpmResult<String> {
        if self.expand_view()? {
            let in_port = self.g.input_port_map().get(port_name);
            let out_port = self.g.output_port_map().get(port_name);

            let port_info = if let Some(port) = in_port {
                port
            } else if let Some(port) = out_port {
                port
            } else {
                return Err(OpossumError::OpticGroup(format!(
                    "port {port_name} is not mapped"
                )));
            };
            let node_id = *self.g.node_by_idx(port_info.0)?.uuid().as_simple();
            Ok(format!("i{}:{}", node_id, port_info.1))
        } else {
            Ok(format!("{node_id}:{port_name}"))
        }
    }
    /// Returns the expansion flag of this [`NodeGroup`].  
    /// If true, the group expands and the internal nodes of this group are displayed in the dot format.
    /// If false, only the group node itself is displayed and the internal setup is not shown
    ///
    /// # Errors
    /// This function returns an error if the property "expand view" does not exist and the
    /// function [`get_bool()`](../properties/struct.Properties.html#method.get_bool) fails
    pub fn expand_view(&self) -> OpmResult<bool> {
        self.node_attr.get_property_bool("expand view")
    }
    /// Define if a [`NodeGroup`] should be displayed expanded or not in diagram.
    ///
    /// # Errors
    /// This function returns an error if the property "expand view" can not be set
    pub fn set_expand_view(&mut self, expand_view: bool) -> OpmResult<()> {
        self.node_attr
            .set_property("expand view", expand_view.into())
    }
    /// Creates the dot format of the [`NodeGroup`] in its expanded view
    /// # Parameters:
    /// * `node_index`:           [`NodeIndex`] of the group
    /// * `name`:                 name of the node
    /// * `inverted`:             boolean that descries wether the node is inverted or not
    ///
    /// Returns the result of the dot string that describes this node
    fn to_dot_expanded_view(
        &self,
        node_index: &str,
        name: &str,
        inverted: bool,
        rankdir: &str,
    ) -> OpmResult<String> {
        let inv_string = if inverted { "(inv)" } else { "" };
        let mut dot_string = format!(
            "  subgraph i{node_index} {{\n\tlabel=\"{name}{inv_string}\"\n\tfontsize=8\n\tcluster=true\n\t"
        );
        dot_string += &self.g.create_dot_string(rankdir)?;
        Ok(dot_string)
    }
    /// Creates the dot format of the [`NodeGroup`] in its collapsed view
    ///
    /// # Parameters:
    /// * `name`:                 name of the node
    /// * `inverted`:             boolean that descries wether the node is inverted or not
    /// * `ports`:               
    ///
    /// Returns the result of the dot string that describes this node
    fn to_dot_collapsed_view(
        &self,
        node_index: &str,
        name: &str,
        inverted: bool,
        ports: &OpticPorts,
        rankdir: &str,
    ) -> String {
        let inv_string = if inverted { " (inv)" } else { "" };
        let node_name = format!("{name}{inv_string}");
        let mut dot_str = format!("\ti{node_index} [\n\t\tshape=plaintext\n");
        let mut indent_level = 2;
        dot_str.push_str(&self.add_html_like_labels(&node_name, &mut indent_level, ports, rankdir));
        dot_str
    }
    /// A helper function for the distances handover between to two [`OpticGraph`]s.
    ///
    /// This function is used during the node positioning procedure and might be removed if a better
    /// solution is found.
    pub fn add_input_port_distance(&mut self, port_name: &str, distance: Length) {
        self.input_port_distances
            .insert(port_name.to_string(), distance);
    }
}

impl Optical for NodeGroup {
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        let ports_to_be_set = self.node_attr.apertures();
        for p in self.g.input_port_map().port_names() {
            ports.create_input(&p).unwrap();
        }
        for p in self.g.output_port_map().port_names() {
            ports.create_output(&p).unwrap();
        }
        if self.g.is_inverted() {
            ports.set_inverted(true);
        }
        ports.set_apertures(ports_to_be_set.clone()).unwrap();
        ports
    }
    fn calc_node_position(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        // set stored distances from predecessors
        self.g
            .set_external_distances(self.input_port_distances.clone());
        let name = format!("group '{}'", self.name());
        self.g.calc_node_positions(&name, &incoming_data)
    }
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let name = format!("Group '{}'", self.name());
        self.g.analyze(&name, &incoming_data, analyzer_type)
    }
    fn as_group(&mut self) -> OpmResult<&mut NodeGroup> {
        Ok(self)
    }
    fn after_deserialization_hook(&mut self) -> OpmResult<()> {
        // Synchronize properties with (internal) graph structure.
        if let Proptype::OpticGraph(g) = &self.node_attr.get_property("graph")? {
            self.g = g.clone();
        }
        Ok(())
    }
    fn report(&self, uuid: &str) -> Option<NodeReport> {
        let mut group_props = Properties::default();
        for node in self
            .g
            .nodes()
            .into_iter()
            .filter(|node| node.optical_ref.borrow().is_detector())
        {
            let sub_uuid = node.uuid().as_simple().to_string();
            if let Some(node_report) = node.optical_ref.borrow().report(&sub_uuid) {
                let node_name = &node.optical_ref.borrow().name();
                if !(group_props.contains(node_name)) {
                    group_props
                        .create(node_name, "", None, node_report.into())
                        .unwrap();
                }
            }
        }
        Some(NodeReport::new(
            &self.node_type(),
            &self.name(),
            uuid,
            group_props,
        ))
    }
    fn is_detector(&self) -> bool {
        self.g.contains_detector()
    }
    fn export_data(&self, report_dir: &Path, _uuid: &str) -> OpmResult<()> {
        let detector_nodes = self
            .g
            .nodes()
            .into_iter()
            .filter(|node| node.optical_ref.borrow().is_detector());
        for node in detector_nodes {
            let uuid = node.uuid().as_simple().to_string();
            node.optical_ref.borrow().export_data(report_dir, &uuid)?;
        }
        Ok(())
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn set_isometry(&mut self, isometry: Isometry) {
        self.node_attr.set_isometry(isometry);
    }
    fn set_global_conf(&mut self, global_conf: Option<Rc<RefCell<SceneryResources>>>) {
        let node_attr = self.node_attr_mut();
        node_attr.set_global_conf(global_conf.clone());
        self.g.update_global_config(&global_conf);
    }
    fn set_inverted(&mut self, inverted: bool) -> OpmResult<()> {
        self.g.set_is_inverted(true);
        self.node_attr_mut().set_inverted(inverted);
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
        rankdir: &str,
    ) -> OpmResult<String> {
        let mut cloned_self = self.clone();
        if self.node_attr.inverted() {
            cloned_self.g.invert_graph()?;
        }
        if self.expand_view()? {
            cloned_self.to_dot_expanded_view(node_index, name, inverted, rankdir)
        } else {
            Ok(cloned_self.to_dot_collapsed_view(node_index, name, inverted, ports, rankdir))
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
        nodes::{test_helper::test_helper::*, Detector, Dummy},
        optical::Optical,
    };
    use num::Zero;
    #[test]
    fn default() {
        let mut node = NodeGroup::default();
        assert_eq!(node.name(), "group");
        assert_eq!(node.node_type(), "group");
        assert_eq!(node.node_attr().inverted(), false);
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
    fn inverted() {
        test_inverted::<NodeGroup>()
    }
    #[test]
    fn ports() {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();
        og.connect_nodes(sn1_i, "rear", sn2_i, "front", Length::zero())
            .unwrap();
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
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();
        og.connect_nodes(sn1_i, "rear", sn2_i, "front", Length::zero())
            .unwrap();
        og.map_input_port(sn1_i, "front", "input").unwrap();
        og.map_output_port(sn2_i, "rear", "output").unwrap();
        og.set_inverted(true).unwrap();
        assert!(og.ports().output_names().contains(&("input".to_string())));
        assert!(og.ports().input_names().contains(&("output".to_string())));
    }
    #[test]
    fn report_default() {
        let group = NodeGroup::default();
        assert!(group.report("").is_some());
        let report = group.report("").unwrap();
        let nr_of_props = report.properties().iter().fold(0, |s: usize, _| s + 1);
        assert_eq!(nr_of_props, 0);
    }
}
