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
    lightdata::{
        light_data_builder::LightDataBuilder, ray_data_builder::RayDataBuilder, LightData,
    },
    optic_node::OpticNode,
    optic_ports::{OpticPorts, PortType},
    optic_ref::OpticRef,
    properties::{Properties, Proptype},
    rays::Rays,
    reporting::{analysis_report::AnalysisReport, node_report::NodeReport},
    surface::optic_surface::OpticSurface,
    SceneryResources,
};
use num::Zero;
pub use optic_graph::OpticGraph;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, File},
    io::Write,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
};
use uom::si::f64::Length;
use uuid::Uuid;
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
///   scenery.connect_nodes(node1, "output_1", node2, "input_1", millimeter!(100.0))?;
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
    node_attr: NodeAttr,
    graph: OpticGraph,
    #[serde(skip)]
    input_port_distances: BTreeMap<String, Length>,
    #[serde(skip)]
    accumulated_rays: Vec<HashMap<Uuid, Rays>>,
}
impl Default for NodeGroup {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("group");
        node_attr
            .create_property(
                "expand view",
                "show group fully expanded in dot diagram?",
                false.into(),
            )
            .unwrap();
        Self {
            graph: OpticGraph::default(),
            input_port_distances: BTreeMap::default(),
            node_attr,
            accumulated_rays: Vec::<HashMap<Uuid, Rays>>::new(),
        }
    }
}

unsafe impl Send for NodeGroup {}

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
    /// consumed (owned) by the [`NodeGroup`]. This function returns a unique id [`Uuid`] as to the element in the scenery.
    /// This reference must be used later on for connecting nodes (see `connect_nodes` function).
    ///
    /// # Errors
    /// An error is returned if the [`NodeGroup`] is set as inverted (which would lead to strange behaviour).
    ///
    /// # Panics
    /// This function panics if the property `graph` can not be updated. Produces an error of type [`OpossumError::Properties`]
    pub fn add_node<T: Analyzable + Clone + 'static>(&mut self, node: T) -> OpmResult<Uuid> {
        let node_id = self.graph.add_node(node)?;
        // save uuid of node in rays if present
        self.store_node_uuid_in_rays_bundle(node_id)?;
        Ok(node_id)
    }
    /// Adds a node to the graph by reference.
    ///
    /// This command adds an [`OpticNode`] by reference but does not connect it to existing nodes in the (sub-)graph. The given node is
    /// consumed (owned) by the [`NodeGroup`]. This function returns the UUID of the node.
    ///
    /// # Errors
    /// An error is returned if the [`NodeGroup`] is set as inverted (which would lead to strange behaviour).
    ///
    /// # Panics
    /// This function panics if the property `graph` cannot be updated. Produces an error of type [`OpossumError::Properties`]
    ///
    /// # Parameters
    /// - `node`: The node to be added by reference.
    ///
    /// # Returns
    /// The UUID of the added node.
    pub fn add_node_ref(&mut self, node: OpticRef) -> OpmResult<Uuid> {
        let uuid = node.uuid();
        self.graph.add_node_ref(node)?;
        // save uuid of node in rays if present
        // self.store_node_uuid_in_rays_bundle(&node.optical_ref.borrow(), idx)?;
        Ok(uuid)
    }
    /// Delete a node from the graph.
    ///
    /// This function deletes a node from the graph. The node is identified by its [`Uuid`]. It also
    /// removes [`NodeReference`](crate::nodes::NodeReference)s the reference the node with the given [`Uuid`].
    ///
    /// The function returns a vector of [`Uuid`]s of the nodes that were deleted. It's a vector because it
    /// contains the original `node_id` and all ids of the possible
    /// [`NodeReference`](crate::nodes::NodeReference)s that were deleted.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the node does not exist.
    /// - the graph is inverted.
    pub fn delete_node(&mut self, node_id: Uuid) -> OpmResult<Vec<Uuid>> {
        self.graph.delete_node(node_id)
    }
    fn store_node_uuid_in_rays_bundle(&self, node_id: Uuid) -> OpmResult<()> {
        let node_ref = self.graph.node(node_id)?;
        let node = node_ref
            .optical_ref
            .lock()
            .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?;
        let Ok(node_props) = node.node_attr().get_property("light data") else {
            return Ok(());
        };
        let node_props = node_props.clone();
        drop(node);
        if let Proptype::LightData(Some(LightData::Geometric(rays))) = node_props {
            let mut new_rays = rays;
            new_rays.set_node_origin_uuid(node_id);

            let mut node_ref = node_ref
                .optical_ref
                .lock()
                .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?;
            node_ref.node_attr_mut().set_property(
                "light data",
                Some(LightDataBuilder::Geometric(RayDataBuilder::Raw(new_rays))).into(),
            )?;
        }
        Ok(())
    }
    /// Return a reference to the optical node specified by its [`Uuid`].
    ///
    /// This function is mainly useful for setting up a [reference node](crate::nodes::NodeReference).
    ///
    /// # Errors
    ///
    /// This function will return [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node(&self, node_id: Uuid) -> OpmResult<OpticRef> {
        self.graph.node(node_id)
    }
    /// Return a reference to the optical node specified by its [`Uuid`].
    ///
    /// This function is similar to [`node`](NodeGroup::node()), but it also recursively searches
    /// for the node in the subnodes of the group.
    ///
    /// # Errors
    ///
    /// This function will return [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node_recursive(&self, node_id: Uuid) -> OpmResult<OpticRef> {
        self.graph.node_recursive(node_id)
    }
    /// Returns all nodes of this [`NodeGroup`].
    #[must_use]
    pub fn nodes(&self) -> Vec<&OpticRef> {
        self.graph.nodes()
    }
    /// Returns the number of nodes of this [`NodeGroup`].
    #[must_use]
    pub fn nr_of_nodes(&self) -> usize {
        self.graph.node_count()
    }
    ///  Connect (already existing) optical nodes within this [`NodeGroup`].
    ///
    /// This function connects two optical nodes (referenced by their [`Uuid`]) with their respective port names
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
    pub fn connect_nodes(
        &mut self,
        src_id: Uuid,
        src_port: &str,
        target_id: Uuid,
        target_port: &str,
        distance: Length,
    ) -> OpmResult<()> {
        self.graph
            .connect_nodes(src_id, src_port, target_id, target_port, distance)
    }
    /// Disconnect two optical nodes within this [`NodeGroup`].
    ///
    /// This function deletes the connection between two nodes, referenced by the [`Uuid`] of the
    /// source node and the name of the source port. **Note**: It's not necessary to specify the target node,
    /// as the connection is uniquely identified by the source node and the source port.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///  - the node with the given [`Uuid`] does not exist.
    ///  - the node's given port is not connected.
    pub fn disconnect_nodes(&mut self, src_id: Uuid, src_port: &str) -> OpmResult<()> {
        self.graph.disconnect_nodes(src_id, src_port)
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
        input_node: Uuid,
        internal_name: &str,
        external_name: &str,
    ) -> OpmResult<()> {
        self.graph
            .map_port(input_node, &PortType::Input, internal_name, external_name)
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
        output_node: Uuid,
        internal_name: &str,
        external_name: &str,
    ) -> OpmResult<()> {
        self.graph
            .map_port(output_node, &PortType::Output, internal_name, external_name)
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
            Ok(format!("i{}:{}", port_info.0.as_simple(), port_info.1))
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
    /// Returns a mutable reference to the underlying [`OpticGraph`] of this [`NodeGroup`].
    #[must_use]
    pub const fn graph(&self) -> &OpticGraph {
        &self.graph
    }
    /// Generate a (top level) [`AnalysisReport`] containing the result of a previously preformed analysis.
    ///
    /// This [`AnalysisReport`] can then be used to either save it to disk or produce an HTML document from. In addition,
    /// the given report folder is used for the individual nodes to export specific result files.
    /// # Errors
    /// This function will return an error if the individual export function of a node fails.
    pub fn toplevel_report(&self) -> OpmResult<AnalysisReport> {
        let mut analysis_report = AnalysisReport::default();
        analysis_report.add_scenery(self);
        let mut section_number: usize = 0;
        for node_ref in self.graph.nodes() {
            let uuid = node_ref.uuid().as_simple().to_string();
            let node_report = node_ref
                .optical_ref
                .lock()
                .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?
                .node_report(&uuid);
            if let Some(mut node_report) = node_report {
                if section_number.is_zero() {
                    node_report.set_show_item(true);
                }
                analysis_report.add_node_report(node_report);
                section_number += 1;
            }
        }
        Ok(analysis_report)
    }
    /// Returns the dot-file header of this [`NodeGroup`] graph.
    fn add_dot_header(&self, rankdir: &str) -> String {
        let mut dot_string = String::from("digraph {\n\tfontsize = 10;\n");
        dot_string.push_str("\tcompound = true;\n");
        dot_string.push_str(&format!("\trankdir = \"{rankdir}\";\n"));
        dot_string.push_str(&format!("\tlabel=\"{}\"\n", self.node_attr.name()));
        dot_string.push_str("\tfontname=\"Courier-monospace\"\n");
        dot_string.push_str("\tnode [fontname=\"Courier-monospace\" fontsize = 10]\n");
        dot_string.push_str("\tedge [fontname=\"Courier-monospace\" fontsize = 10]\n\n");
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
    pub fn toplevel_dot_svg(&self, dot_str_file: &PathBuf, svg_file: &mut File) -> OpmResult<()> {
        let dot_string = fs::read_to_string(dot_str_file)
            .map_err(|e| OpossumError::Other(format!("writing diagram file (.svg) failed: {e}")))?;
        let svg_str = Self::dot_string_to_svg_str(dot_string.as_str())?;
        write!(svg_file, "{svg_str}")
            .map_err(|e| OpossumError::Other(format!("writing diagram file (.svg) failed: {e}")))
    }

    /// Converts a dot string to an svg string
    /// # Attributes
    /// `dot_string`: string that constains the dot information
    /// # Errors
    /// This function errors if
    /// - the spawn of a childprocess fails
    /// - the mutable stdin handle creation fails
    /// - writing to child stdin fails
    /// - output collection fails
    /// - string to utf8 conversion fails
    fn dot_string_to_svg_str(dot_string: &str) -> OpmResult<String> {
        let mut child = std::process::Command::new("dot")
            .arg("-Tsvg:cairo")
            .arg("-Kdot")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;

        let Some(child_stdin) = child.stdin.as_mut() else {
            return Err(OpossumError::Other(
                "conversion to image failed: could not set stdin for graphviz command".into(),
            ));
        };
        child_stdin
            .write_all(dot_string.as_bytes())
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;

        let output = child
            .wait_with_output()
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;

        let svg_string = String::from_utf8(output.stdout)
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;
        Ok(svg_string)
    }
    /// Returns a reference to the accumulated rays of this [`NodeGroup`].
    ///
    /// This function returns a bundle of all rays that propagated in a group after a ghost focus analysis.
    /// This function is in particular helpful for generating a global ray propagation plot.
    #[must_use]
    pub const fn accumulated_rays(&self) -> &Vec<HashMap<Uuid, Rays>> {
        &self.accumulated_rays
    }

    /// add a ray bundle to the set of accumulated rays of this node group
    /// # Arguments
    /// - rays: pointer to ray bundle that should be included
    /// - bounce: bouncle level of these rays
    pub fn add_to_accumulated_rays(&mut self, rays: &Rays, bounce: usize) {
        if self.accumulated_rays.len() <= bounce {
            let mut hashed_rays = HashMap::<Uuid, Rays>::new();
            hashed_rays.insert(rays.uuid(), rays.clone());
            self.accumulated_rays.push(hashed_rays);
        } else {
            self.accumulated_rays[bounce].insert(rays.uuid(), rays.clone());
        }
    }

    /// Clears the edges of a graph. Necessary for ghost focus analysis.
    pub fn clear_edges(&mut self) {
        self.graph.clear_edges();
    }
    /// Sets the graph of this [`NodeGroup`].
    ///
    /// This function shoud be used with caution. It is mainly used for deserialization purposes.
    pub fn set_graph(&mut self, graph: OpticGraph) {
        self.graph = graph;
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
    fn as_group_mut(&mut self) -> OpmResult<&mut NodeGroup> {
        Ok(self)
    }
    fn after_deserialization_hook(&mut self) -> OpmResult<()> {
        self.graph.set_is_inverted(self.node_attr.inverted());
        Ok(())
    }
    fn node_report(&self, uuid: &str) -> Option<NodeReport> {
        let mut group_props = Properties::default();
        for node in self.graph.nodes() {
            let sub_uuid = node.uuid().as_simple().to_string();
            if let Ok(node_ref) = node.optical_ref.lock() {
                if let Some(node_report) = node_ref.node_report(&sub_uuid) {
                    let node_name = node_ref.name();
                    if !(group_props.contains(&node_name)) {
                        group_props
                            .create(&node_name, "", node_report.into())
                            .unwrap();
                    }
                }
            }
        }
        if group_props.is_empty() {
            None
        } else {
            Some(NodeReport::new(
                &self.node_type(),
                &self.name(),
                uuid,
                group_props,
            ))
        }
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn set_global_conf(&mut self, global_conf: Option<Arc<Mutex<SceneryResources>>>) {
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
            if let Ok(mut node) = node.optical_ref.lock() {
                node.reset_data();
            }
        }
        self.accumulated_rays = Vec::<HashMap<Uuid, Rays>>::new();
    }
    fn get_optic_surface_mut(&mut self, _surf_name: &str) -> Option<&mut OpticSurface> {
        None
    }

    fn update_surfaces(&mut self) -> OpmResult<()> {
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
            cloned_self.graph.invert_graph()?;
        }
        if self.expand_view()? {
            cloned_self.to_dot_expanded_view(node_index, name, inverted, rankdir)
        } else {
            Ok(cloned_self.to_dot_collapsed_view(node_index, name, inverted, ports, rankdir))
        }
    }
    fn node_color(&self) -> &'static str {
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
        lightdata::{light_data_builder::LightDataBuilder, ray_data_builder::RayDataBuilder},
        millimeter, nanometer,
        nodes::{test_helper::test_helper::*, Dummy, EnergyMeter, Source},
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
        assert!(node.as_group_mut().is_ok());
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
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
            .unwrap();
        assert!(og.ports().names(&PortType::Input).is_empty());
        assert!(og.ports().names(&PortType::Output).is_empty());
        og.map_input_port(sn1_i, "input_1", "input_1").unwrap();
        assert!(og
            .ports()
            .names(&PortType::Input)
            .contains(&("input_1".to_string())));
        og.map_output_port(sn2_i, "output_1", "output_1").unwrap();
        assert!(og
            .ports()
            .names(&PortType::Output)
            .contains(&("output_1".to_string())));
    }
    #[test]
    fn ports_inverted() {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
            .unwrap();
        og.map_input_port(sn1_i, "input_1", "input_1").unwrap();
        og.map_output_port(sn2_i, "output_1", "output_1").unwrap();
        og.set_inverted(true).unwrap();
        assert!(og
            .ports()
            .names(&PortType::Output)
            .contains(&("input_1".to_string())));
        assert!(og
            .ports()
            .names(&PortType::Input)
            .contains(&("output_1".to_string())));
    }
    #[test]
    fn report() {
        let mut scenery = NodeGroup::default();
        scenery.add_node(Dummy::default()).unwrap();
        let report = scenery.toplevel_report().unwrap();
        assert!(ron::ser::to_string_pretty(&report, ron::ser::PrettyConfig::default()).is_ok());
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
            .connect_nodes(node1, "output_1", node2, "input_1", Length::zero())
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
        let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::Raw(rays));
        let i_s = scenery
            .add_node(Source::new("src", light_data_builder))
            .unwrap();
        let mut em = EnergyMeter::default();
        em.set_isometry(Isometry::identity()).unwrap();
        let i_e = scenery.add_node(em).unwrap();
        scenery
            .connect_nodes(i_s, "output_1", i_e, "input_1", Length::zero())
            .unwrap();
        let mut raytrace_config = RayTraceConfig::default();
        raytrace_config.set_min_energy_per_ray(joule!(0.5)).unwrap();
        AnalysisRayTrace::analyze(&mut scenery, LightResult::default(), &raytrace_config).unwrap();
        let uuid = scenery.node(i_e).unwrap().uuid().as_simple().to_string();
        let report = scenery
            .node(i_e)
            .unwrap()
            .optical_ref
            .lock()
            .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))
            .unwrap()
            .node_report(&uuid)
            .unwrap();
        if let Proptype::Energy(e) = report.properties().get("Energy").unwrap() {
            assert_eq!(e, &joule!(1.0));
        } else {
            assert!(false)
        }
    }
}
