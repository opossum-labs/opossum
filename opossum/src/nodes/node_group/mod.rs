#![warn(missing_docs)]
mod analysis_energy;
mod analysis_ghostfocus;
mod analysis_raytrace;
mod optic_graph;
use super::node_attr::NodeAttr;
use crate::{
    analyzers::Analyzable,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    get_version,
    optic_node::OpticNode,
    optic_ports::{OpticPorts, PortType},
    optic_ref::OpticRef,
    plottable::{Plottable, PltBackEnd},
    properties::{Properties, Proptype},
    reporting::reporter::{AnalysisReport, NodeReport},
    SceneryResources,
};
use chrono::Local;
use log::{info, warn};
pub use optic_graph::OpticGraph;
use petgraph::prelude::NodeIndex;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    rc::Rc,
};
use tempfile::NamedTempFile;
use uom::si::f64::Length;
#[derive(Debug, Clone, Serialize, Deserialize)]
/// The basic building block of an optical system. It represents a group of other optical
/// nodes ([`OpticNode`]s) arranged in a (sub)graph.
///
/// # Example
///
/// ```rust
/// use opossum::nodes::{NodeGroup, Dummy};
/// use opossum::error::OpmResult;
/// use opossum::millimeter;
///
/// fn main() -> OpmResult<()> {
///   let mut scenery = NodeGroup::new("OpticScenery demo");
///   let node1 = scenery.add_node(Dummy::new("dummy1"))?;
///   let node2 = scenery.add_node(Dummy::new("dummy2"))?;
///   scenery.connect_nodes(node1, "rear", node2, "front", millimeter!(100.0))?;
///   Ok(())
/// }
///
/// ```
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
    graph: OpticGraph,
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
            graph: OpticGraph::default(),
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
    /// Add a given [`OpticNode`] to the (sub-)graph of this [`NodeGroup`].
    ///
    /// This command just adds an [`OpticNode`] but does not connect it to existing nodes in the (sub-)graph. The given node is
    /// consumed (owned) by the [`NodeGroup`]. This function returns a reference to the element in the scenery as [`NodeIndex`].
    /// This reference must be used later on for connecting nodes (see `connect_nodes` function).
    ///
    /// # Errors
    /// An error is returned if the [`NodeGroup`] is set as inverted (which would lead to strange behaviour).
    ///
    /// # Panics
    /// This function panics if the property "graph" can not be updated. Produces an error of type [`OpossumError::Properties`]
    pub fn add_node<T: Analyzable + 'static>(&mut self, node: T) -> OpmResult<NodeIndex> {
        let idx = self.graph.add_node(node);
        self.node_attr
            .set_property("graph", self.graph.clone().into())
            .unwrap();
        idx
    }
    /// Return a reference to the optical node specified by its [`NodeIndex`].
    ///
    /// This function is mainly useful for setting up a [reference node](crate::nodes::NodeReference).
    ///
    /// # Errors
    ///
    /// This function will return [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node(&self, node_idx: NodeIndex) -> OpmResult<OpticRef> {
        self.graph.node_by_idx(node_idx)
    }
    ///  Connect (already existing) optical nodes within this [`NodeGroup`].
    ///
    /// This function connects two optical nodes (referenced by their [`NodeIndex`]) with their respective port names
    /// and their geometrical distance (= propagation length) to each other thus extending the optical network.
    /// **Note**: The connection of two internal nodes might affect external port mappings (see [`map_input_port`](NodeGroup::map_input_port())
    /// & [`map_output_port`](NodeGroup::map_output_port()) functions). In this case no longer valid mappings will be deleted.
    ///
    /// # Errors
    /// This function returns an [`OpossumError::OpticScenery`] if
    ///   - the group is set as `inverted`. Connectiing subnodes of an inverted group node would result in strange behaviour.
    ///   - the source node / port or target node / port does not exist.
    ///   - the source node / port or target node / port is already connected.
    ///   - the node connection would form a loop in the graph.
    ///
    /// In addition this function returns an [`OpossumError::Properties`] if the (internal) property "graph" cannot be set.
    pub fn connect_nodes(
        &mut self,
        src_node: NodeIndex,
        src_port: &str,
        target_node: NodeIndex,
        target_port: &str,
        distance: Length,
    ) -> OpmResult<()> {
        self.graph
            .connect_nodes(src_node, src_port, target_node, target_port, distance)?;
        self.node_attr
            .set_property("graph", self.graph.clone().into())?;
        Ok(())
    }
    /// Map an input port of an internal node to an external port of the group.
    ///
    /// In oder to use a [`NodeGroup`] from the outside, internal nodes / ports must be mapped to be visible. The
    /// corresponding [`ports`](NodeGroup::ports()) function only returns ports that have been mapped before.
    /// # Errors
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
        self.graph
            .map_port(input_node, &PortType::Input, internal_name, external_name)?;
        self.node_attr
            .set_property("graph", self.graph.clone().into())
    }
    /// Map an output port of an internal node to an external port of the group.
    ///
    /// In oder to use a [`NodeGroup`] from the outside, internal nodes / ports must be mapped to be visible. The
    /// corresponding [`ports`](NodeGroup::ports()) function only returns ports that have been mapped before.
    /// # Errors
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
        self.graph
            .map_port(output_node, &PortType::Output, internal_name, external_name)?;
        self.node_attr
            .set_property("graph", self.graph.clone().into())
    }
    /// Defines and returns the node/port identifier to connect the edges in the dot format
    /// # Parameters
    ///   - `port_name`:            name of the external port of the group
    ///   - `node_id`:    String containing the uuid of the parent node
    /// # Errors
    /// Returns [`OpossumError::OpticGroup`], if the specified `port_name` is not mapped as input or output
    pub fn get_mapped_port_str(&self, port_name: &str, node_id: &str) -> OpmResult<String> {
        if self.expand_view()? {
            let in_port = self.graph.port_map(&PortType::Input).get(port_name);
            let out_port = self.graph.port_map(&PortType::Output).get(port_name);

            let port_info = if let Some(port) = in_port {
                port
            } else if let Some(port) = out_port {
                port
            } else {
                return Err(OpossumError::OpticGroup(format!(
                    "port {port_name} is not mapped"
                )));
            };
            let node_id = *self.graph.node_by_idx(port_info.0)?.uuid().as_simple();
            Ok(format!("i{}:{}", node_id, port_info.1))
        } else {
            Ok(format!("{node_id}:{port_name}"))
        }
    }
    /// Returns the expansion flag of this [`NodeGroup`].  
    /// If true, the group expands and the internal nodes of this group are displayed in the dot format.
    /// If false, only the group node itself is displayed and the internal setup is not shown
    /// # Errors
    /// This function returns an error if the property "expand view" does not exist and the
    /// function [`get_bool()`](../properties/struct.Properties.html#method.get_bool) fails
    pub fn expand_view(&self) -> OpmResult<bool> {
        self.node_attr.get_property_bool("expand view")
    }
    /// Define if a [`NodeGroup`] should be displayed expanded or not in diagram.
    /// # Errors
    /// This function returns an error if the property "expand view" can not be set
    pub fn set_expand_view(&mut self, expand_view: bool) -> OpmResult<()> {
        self.node_attr
            .set_property("expand view", expand_view.into())
    }
    /// Creates the dot format of the [`NodeGroup`] in its expanded view
    /// # Parameters:
    ///   - `node_index`: [`NodeIndex`] of the group
    ///   - `name`:       name of the node
    ///   - `inverted`:   boolean that descries wether the node is inverted or not
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
        dot_string += &self.graph.create_dot_string(rankdir)?;
        Ok(dot_string)
    }
    /// Creates the dot format of the [`NodeGroup`] in its collapsed view
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
    /// A helper function for the distances handover between to two `OpticGraph`s.
    ///
    /// This function is used during the node positioning procedure and might be removed if a better
    /// solution is found.
    pub fn add_input_port_distance(&mut self, port_name: &str, distance: Length) {
        self.input_port_distances
            .insert(port_name.to_string(), distance);
    }
    /// Returns a mutable reference to the underlying [`OpticGraph`] of this [`NodeGroup`].
    pub fn graph_mut(&mut self) -> &mut OpticGraph {
        &mut self.graph
    }
    /// Write node specific data files to the given `data_dir`.
    /// # Errors
    /// This function will return an error if the underlying `export_data` function of the corresponding
    /// node returns an error.
    pub fn export_node_data(&self, data_dir: &Path) -> OpmResult<()> {
        for node in self.graph.nodes() {
            let uuid = node.uuid().as_simple().to_string();
            node.optical_ref.borrow().export_data(data_dir, &uuid)?;
        }
        Ok(())
    }
    /// Generate a (top level) [`AnalysisReport`] containing the result of a previously preformed analysis.
    ///
    /// This [`AnalysisReport`] can then be used to either save it to disk or produce an HTML document from. In addition,
    /// the given report folder is used for the individual nodes to export specific result files.
    /// # Errors
    /// This function will return an error if the individual export function of a node fails.
    pub fn toplevel_report(&self) -> OpmResult<AnalysisReport> {
        let mut analysis_report = AnalysisReport::new(get_version(), Local::now());
        analysis_report.add_scenery(self);
        for node in self.graph.nodes() {
            //detector_nodes {
            let node_name = &node.optical_ref.borrow().name();
            info!("toplevel report data for node {node_name}");
            let uuid = node.uuid().as_simple().to_string();
            if let Some(node_report) = node.optical_ref.borrow().node_report(&uuid) {
                analysis_report.add_node_report(node_report);
            }
        }
        Ok(analysis_report)
    }
    /// Returns the dot-file header of this [`NodeGroup`] graph.
    fn add_dot_header(&self, rankdir: &str) -> String {
        let mut dot_string = String::from("digraph {\n\tfontsize = 8;\n");
        dot_string.push_str("\tcompound = true;\n");
        dot_string.push_str(&format!("\trankdir = \"{rankdir}\";\n"));
        dot_string.push_str(&format!("\tlabel=\"{}\"\n", self.node_attr.name()));
        dot_string.push_str("\tfontname=\"Courier-monospace\"\n");
        dot_string.push_str("\tnode [fontname=\"Courier-monospace\" fontsize = 8]\n");
        dot_string.push_str("\tedge [fontname=\"Courier-monospace\"]\n\n");
        dot_string
    }
    /// Export the optic graph, including ports, into the `dot` format to be used in combination with
    /// the [`graphviz`](https://graphviz.org/) software.
    ///
    /// # Errors
    /// This function returns an error if nodes do not return a proper value for their `name` property.
    pub fn toplevel_dot(&self, rankdir: &str) -> OpmResult<String> {
        let mut dot_string = self.add_dot_header(rankdir);
        dot_string += &self.graph.create_dot_string(rankdir)?;
        Ok(dot_string)
    }
    /// Generate an SVG of the (top level) [`NodeGroup`] `dot` diagram.
    ///
    /// This function returns a string of a SVG image (scalable vector graphics). This string can be directly written to a
    /// `*.svg` file.
    /// # Errors
    ///
    /// This function will return an error if the image generation fails (e.g. program not found, no memory left etc.).
    pub fn toplevel_dot_svg(&self) -> OpmResult<String> {
        let dot_string = self.toplevel_dot("")?;
        let mut f = NamedTempFile::new()
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;
        f.write_all(dot_string.as_bytes())
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;
        let r = std::process::Command::new("dot")
            .arg(f.path())
            .arg("-Tsvg:cairo")
            .arg("-Kdot")
            .output()
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;
        let svg_string = String::from_utf8(r.stdout)
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;
        Ok(svg_string)
    }
}

impl OpticNode for NodeGroup {
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        let ports_to_be_set = self.node_attr.ports();
        for p in self.graph.port_map(&PortType::Input).port_names() {
            ports.add(&PortType::Input, &p).unwrap();
        }
        for p in self.graph.port_map(&PortType::Output).port_names() {
            ports.add(&PortType::Output, &p).unwrap();
        }
        if self.graph.is_inverted() {
            ports.set_inverted(true);
        }
        ports.set_apertures(ports_to_be_set.clone()).unwrap();
        ports
    }
    fn as_group(&mut self) -> OpmResult<&mut NodeGroup> {
        Ok(self)
    }
    fn after_deserialization_hook(&mut self) -> OpmResult<()> {
        // Synchronize properties with (internal) graph structure.
        if let Proptype::OpticGraph(g) = &self.node_attr.get_property("graph")? {
            self.graph = g.clone();
        }
        Ok(())
    }
    fn node_report(&self, uuid: &str) -> Option<NodeReport> {
        let mut group_props = Properties::default();
        for node in self.graph.nodes() {
            let sub_uuid = node.uuid().as_simple().to_string();
            if let Some(node_report) = node.optical_ref.borrow().node_report(&sub_uuid) {
                let node_name = &node.optical_ref.borrow().name();
                info!("report data for node {node_name}");
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
    fn export_data(&self, report_dir: &Path, _uuid: &str) -> OpmResult<()> {
        for node in self.graph.nodes() {
            let node_name = node.optical_ref.borrow().name();
            info!("export data for node {node_name}");
            let uuid = node.uuid().as_simple().to_string();
            node.optical_ref.borrow().export_data(report_dir, &uuid)?;
            let hitmaps = node.optical_ref.borrow().hit_maps();
            for hitmap in &hitmaps {
                let port_name = hitmap.0;
                info!("   found hitmap for port {port_name}");
                let file_path = PathBuf::from(report_dir).join(Path::new(&format!(
                    "hitmap_{node_name}_{port_name}_{uuid}.svg"
                )));
                if !hitmap.1.is_empty() {
                    hitmap.1.to_plot(&file_path, PltBackEnd::SVG)?;
                }
            }
        }
        Ok(())
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn set_global_conf(&mut self, global_conf: Option<Rc<RefCell<SceneryResources>>>) {
        let node_attr = self.node_attr_mut();
        node_attr.set_global_conf(global_conf.clone());
        self.graph.update_global_config(&global_conf);
    }
    fn set_inverted(&mut self, inverted: bool) -> OpmResult<()> {
        self.graph.set_is_inverted(inverted);
        self.node_attr_mut().set_inverted(inverted);
        Ok(())
    }
    fn reset_data(&mut self) {
        let nodes = self.graph.nodes();
        for node in nodes {
            node.optical_ref.borrow_mut().reset_data();
        }
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
            cloned_self.graph.invert_graph()?;
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
impl Analyzable for NodeGroup {}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{energy::AnalysisEnergy, raytrace::AnalysisRayTrace, RayTraceConfig},
        joule,
        light_result::LightResult,
        lightdata::LightData,
        millimeter, nanometer,
        nodes::{test_helper::test_helper::*, Detector, Dummy, EnergyMeter, Source},
        optic_node::OpticNode,
        ray::Ray,
        rays::Rays,
        utils::geom_transformation::Isometry,
    };
    use num::Zero;
    #[test]
    fn default() {
        let mut node = NodeGroup::default();
        assert_eq!(node.name(), "group");
        assert_eq!(node.node_type(), "group");
        assert_eq!(node.node_attr().inverted(), false);
        assert_eq!(node.expand_view().unwrap(), false);
        assert_eq!(node.node_color(), "yellow");
        assert!(node.as_group().is_ok());
        assert_eq!(node.graph.edge_count(), 0);
        assert_eq!(node.graph.node_count(), 0);
    }
    #[test]
    fn expand_view_property() {
        let mut node = NodeGroup::default();
        node.set_expand_view(true).unwrap();
        assert_eq!(node.expand_view().unwrap(), true);
        node.set_expand_view(false).unwrap();
        assert_eq!(node.expand_view().unwrap(), false);
    }
    #[test]
    fn new() {
        let node = NodeGroup::new("test");
        assert_eq!(node.name(), "test");
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
        assert!(og.ports().names(&PortType::Input).is_empty());
        assert!(og.ports().names(&PortType::Output).is_empty());
        og.map_input_port(sn1_i, "front", "input").unwrap();
        assert!(og
            .ports()
            .names(&PortType::Input)
            .contains(&("input".to_string())));
        og.map_output_port(sn2_i, "rear", "output").unwrap();
        assert!(og
            .ports()
            .names(&PortType::Output)
            .contains(&("output".to_string())));
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
        assert!(og
            .ports()
            .names(&PortType::Output)
            .contains(&("input".to_string())));
        assert!(og
            .ports()
            .names(&PortType::Input)
            .contains(&("output".to_string())));
    }
    #[test]
    fn report() {
        let mut scenery = NodeGroup::default();
        scenery.add_node(Detector::default()).unwrap();
        let report = scenery.toplevel_report().unwrap();
        assert!(serde_yaml::to_string(&report).is_ok());
        // How shall we further parse the output?
    }
    #[test]
    fn report_empty() {
        let mut scenery = NodeGroup::default();
        AnalysisEnergy::analyze(&mut scenery, LightResult::default()).unwrap();
        scenery.toplevel_report().unwrap();
    }
    #[test]
    fn analyze_dummy() {
        let mut scenery = NodeGroup::default();
        let node1 = scenery.add_node(Dummy::default()).unwrap();
        let node2 = scenery.add_node(Dummy::default()).unwrap();
        scenery
            .connect_nodes(node1, "rear", node2, "front", Length::zero())
            .unwrap();
        AnalysisEnergy::analyze(&mut scenery, LightResult::default()).unwrap();
    }
    #[test]
    fn analyze_empty() {
        let mut scenery = NodeGroup::default();
        AnalysisEnergy::analyze(&mut scenery, LightResult::default()).unwrap();
    }
    #[test]
    fn analyze_energy_threshold() {
        let mut rays = Rays::default();
        rays.add_ray(
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(1.0)).unwrap(),
        );
        rays.add_ray(
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(0.1)).unwrap(),
        );
        let mut scenery = NodeGroup::default();
        let i_s = scenery
            .add_node(Source::new("src", &LightData::Geometric(rays)))
            .unwrap();
        let mut em = EnergyMeter::default();
        em.set_isometry(Isometry::identity());
        let i_e = scenery.add_node(em).unwrap();
        scenery
            .connect_nodes(i_s, "out1", i_e, "in1", Length::zero())
            .unwrap();
        let mut raytrace_config = RayTraceConfig::default();
        raytrace_config.set_min_energy_per_ray(joule!(0.5)).unwrap();
        AnalysisRayTrace::analyze(&mut scenery, LightResult::default(), &raytrace_config).unwrap();
        let uuid = scenery.node(i_e).unwrap().uuid().as_simple().to_string();
        let report = scenery
            .node(i_e)
            .unwrap()
            .optical_ref
            .borrow()
            .node_report(&uuid)
            .unwrap();
        if let Proptype::Energy(e) = report.properties().get("Energy").unwrap() {
            assert_eq!(*e, joule!(1.0));
        } else {
            assert!(false)
        }
    }
}
