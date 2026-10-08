#![warn(missing_docs)]
//! # Node groups
//!
//! A node group is a special type of optical node that can contain other optical nodes (including other groups) and connections between them. It allows to build up complex optical systems in a hierarchical way. The internal structure of a node group can be hidden or shown in the dot format by setting the `expand view` property of the group node. To use a node group from the outside, internal nodes / ports must be mapped to be visible (see [`map_input_port`](NodeGroup::map_input_port()) & [`map_output_port`](NodeGroup::map_output_port()) functions).

use opm_macros_lib::OpmNode;
mod analysis_energy;
mod analysis_ghostfocus;
mod analysis_raytrace;
pub mod optic_graph;
pub mod port_map;
mod undo;
mod visualization;

use crate::{
    analyzers::{AnalyzerKind, propagation_strategy::PropagationStrategy},
    core_optics::{
        NodeAttr, NodeAttrExt, OpticNode, OpticPorts, OpticRef, PortType, node_attr::HasNodeAttr,
    },
    error::{OpmResult, OpossumError},
    geometry::Geometry,
    light::{
        Rays,
        lightdata::{LightData, light_data_builder::LightDataBuilder},
    },
    nodes::{
        NodeRegistration,
        node_group::optic_graph::delta::{GraphDelta, RemovedPortMapping},
    },
    properties::{Properties, Proptype},
    reporting::{
        analysis_report::AnalysisReport,
        node_report::{NodeReport, NodeReportResult},
        report_note::{ReportLevel, ReportNote},
    },
};
pub use optic_graph::{ConnectionInfo, OpticGraph};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use uom::si::f64::Length;
use uuid::Uuid;

inventory::submit! {
    NodeRegistration::new::<NodeGroup>("group", "group node containing other nodes or groups")
}

#[derive(OpmNode, Debug, Clone, Serialize, Deserialize)]
#[manual_analyzable]
/// The basic building block of an optical system. It represents a group of other optical
/// nodes ([`OpticNode`]s) arranged in a (sub)graph.
pub struct NodeGroup {
    #[serde(flatten)]
    node_attr: NodeAttr,
    #[serde(default, skip_serializing_if = "OpticGraph::is_empty")]
    graph: OpticGraph,
    #[serde(skip)]
    input_port_distances: BTreeMap<String, Length>,
    #[serde(skip)]
    accumulated_rays: Vec<HashMap<Uuid, Rays>>,
}

impl Analyzable for NodeGroup {
    fn clone_analyzable(&self) -> Box<dyn Analyzable> {
        Box::new(self.clone())
    }
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

impl NodeGroup {
    /// Creates a new [`NodeGroup`].
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut group = Self::default();
        group.node_attr.set_name(name);
        group
    }

    /// Add a given [`OpticNode`] to the (sub-)graph of this [`NodeGroup`].
    ///
    /// # Errors
    /// Returns an error if the group is set as inverted or the node already exists.
    pub fn add_node<T: Analyzable + Clone + 'static>(&mut self, node: T) -> OpmResult<Uuid> {
        self.add_node_with_delta(node).map(|(node_id, _)| node_id)
    }

    /// Add a given [`OpticNode`] to the (sub-)graph and return its [`Uuid`] along with
    /// a [`GraphDelta`] for transactional undo tracking.
    ///
    /// # Errors
    /// Returns an error if the group is set as inverted or the node already exists.
    pub fn add_node_with_delta<T: Analyzable + Clone + 'static>(
        &mut self,
        node: T,
    ) -> OpmResult<(Uuid, GraphDelta)> {
        let group_id = self.node_attr().uuid();
        let node_id = self.graph.add_node(node)?;
        self.store_node_uuid_in_rays_bundle(node_id)?;
        let node_ref = self.graph.node(node_id)?;

        let delta = GraphDelta::NodeAdded {
            group_id,
            node_id,
            node: node_ref,
        };
        Ok((node_id, delta))
    }

    /// Adds a node to the graph by reference.
    ///
    /// # Errors
    /// Returns an error if the group is set as inverted.
    pub fn add_node_ref(&mut self, node: OpticRef) -> OpmResult<Uuid> {
        self.add_node_ref_with_delta(node)
            .map(|(node_id, _)| node_id)
    }

    /// Adds a node to the graph by reference and returns its [`Uuid`] along with
    /// a [`GraphDelta`] for transactional undo tracking.
    ///
    /// # Errors
    /// Returns an error if the group is set as inverted.
    pub fn add_node_ref_with_delta(&mut self, node: OpticRef) -> OpmResult<(Uuid, GraphDelta)> {
        let group_id = self.node_attr().uuid();
        let uuid = node.uuid();
        self.graph.add_node_ref(node.clone())?;

        let delta = GraphDelta::NodeAdded {
            group_id,
            node_id: uuid,
            node,
        };
        Ok((uuid, delta))
    }

    /// Delete a node from the graph.
    ///
    /// # Errors
    /// Returns an error if the node does not exist or the graph is inverted.
    pub fn delete_node(&mut self, node_id: Uuid) -> OpmResult<GraphDelta> {
        let group_id = self.node_attr().uuid();
        let deletion_delta = self.graph.delete_node_with_delta(node_id, group_id)?;
        Ok(GraphDelta::NodeDeleted(deletion_delta))
    }

    /// Remove a single node from the graph without cascading to reference nodes.
    ///
    /// # Errors
    /// Returns an error if the group is inverted or the node does not exist.
    pub fn remove_node_no_cascade(&mut self, node_id: Uuid) -> OpmResult<()> {
        self.graph.remove_node_no_cascade(node_id)
    }

    /// Recursively collects the UUIDs of all nodes contained in this graph.
    ///
    /// # Errors
    /// Returns an error if acquiring a lock on any contained node fails.
    pub fn collect_all_contained_node_ids_recursive(&self) -> OpmResult<Vec<Uuid>> {
        let mut result = Vec::new();
        for node_ref in self.nodes() {
            let uuid = node_ref.uuid();
            result.push(uuid);
            if let Some(group) = node_ref.as_any().downcast_ref::<Self>() {
                let mut sub_ids = group.collect_all_contained_node_ids_recursive()?;
                result.append(&mut sub_ids);
            }
        }
        Ok(result)
    }

    /// Recursively collects all optical node references contained in this graph.
    ///
    /// # Errors
    /// Returns an error if acquiring a lock on any contained node fails.
    pub fn collect_all_nodes_recursive(&self) -> OpmResult<Vec<OpticRef>> {
        let mut result = Vec::new();
        for node_ref in self.nodes() {
            result.push(node_ref.clone());
            if let Some(group) = node_ref.as_any().downcast_ref::<Self>() {
                let mut sub_nodes = group.collect_all_nodes_recursive()?;
                result.append(&mut sub_nodes);
            }
        }
        Ok(result)
    }

    /// Executes a closure on every optical node in this group and all nested subgroups recursively.
    pub fn for_each_node_mut(&mut self, f: &mut impl FnMut(&mut OpticRef)) {
        for node in self.graph.g.node_weights_mut() {
            f(node);
            if let Some(group) = node.as_any_mut().downcast_mut::<Self>() {
                group.for_each_node_mut(f);
            }
        }
    }

    /// Rebuild the surfaces of every node in this group and its subgroups from the node's current
    /// properties.
    ///
    /// A property can be changed without its node rebuilding its surfaces: the backend writes it
    /// into the node's attributes directly. An analysis that traces rays calls this first, so it
    /// traces each component as its properties state it now.
    ///
    /// # Errors
    ///
    /// This function returns an error if a node cannot build its surfaces.
    pub fn rebuild_surfaces(&mut self) -> OpmResult<()> {
        let mut result = Ok(());
        self.for_each_node_mut(&mut |node| {
            if result.is_ok() {
                result = node.update_surfaces();
            }
        });
        result
    }

    /// Returns the hierarchy of nodes starting from `node_id` bottom-up to the root group.
    ///
    /// # Errors
    /// Returns an error if node resolution or lock acquisition fails.
    pub fn get_node_hierarchy_bottom_up(&self, node_id: Uuid) -> OpmResult<Vec<(Uuid, String)>> {
        let mut group_hierarchy = Vec::<(Uuid, String)>::new();
        self.with_node_attr(node_id, |node_attr| {
            group_hierarchy.push((node_id, node_attr.name().to_string()));
        })?;

        if self.node_attr().uuid() != node_id {
            let parent_id = self.node_recursive(node_id)?.1;
            let group_vec = self.get_node_hierarchy_bottom_up(parent_id).map_err(|e| {
                OpossumError::OpticGroup(format!("Error getting node hierarchy: {e}"))
            })?;
            group_hierarchy.extend(group_vec);
        }
        Ok(group_hierarchy)
    }

    fn store_node_uuid_in_rays_bundle(&mut self, node_id: Uuid) -> OpmResult<()> {
        let node_ref = self.graph.node_mut(node_id)?;
        let Ok(node_props) = node_ref.node_attr().get_property("light data") else {
            return Ok(());
        };
        let node_props = node_props.clone();
        if let Proptype::LightData(Some(LightData::Geometric(rays))) = node_props {
            let mut new_rays = rays;
            new_rays.set_node_origin_uuid(node_id);
            node_ref.node_attr_mut().set_property(
                "light data",
                LightDataBuilder::Geometric(new_rays.into()).into(),
            )?;
        }
        Ok(())
    }

    /// Return a reference to the optical node specified by its [`Uuid`].
    ///
    /// # Errors
    /// Returns [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node(&self, node_id: Uuid) -> OpmResult<OpticRef> {
        if node_id == self.node_attr.uuid() {
            Ok(OpticRef::new(Box::new(self.clone())))
        } else {
            self.graph.node(node_id)
        }
    }

    /// Return `true` if a node with the given [`Uuid`] exists in the graph.
    #[must_use]
    pub fn exists(&self, node_id: Uuid) -> bool {
        self.node_recursive(node_id).is_ok()
    }

    /// Return a reference to the node specified by `node_id` and the UUID of its parent group.
    ///
    /// # Errors
    /// Returns [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node_recursive(&self, node_id: Uuid) -> OpmResult<(OpticRef, Uuid)> {
        self.graph.node_recursive(node_id, self.node_attr().uuid())
    }

    /// Execute a read-only operation on the `NodeGroup` identified by `node_id`.
    ///
    /// # Errors
    /// Returns an error if the node does not exist, cannot be downcast, or locking fails.
    pub fn with_group_node<R>(&self, node_id: Uuid, f: impl FnOnce(&Self) -> R) -> OpmResult<R> {
        if self.node_attr().uuid() == node_id {
            return Ok(f(self));
        }
        let (node_ref, _) = self.node_recursive(node_id)?;
        let Some(group) = node_ref.as_any().downcast_ref::<Self>() else {
            return Err(OpossumError::Other("could not cast to NodeGroup".into()));
        };
        Ok(f(group))
    }

    /// Execute a mutable operation on the `NodeGroup` identified by `node_id`.
    ///
    /// # Errors
    /// Returns an error if the node does not exist, cannot be downcast, or locking fails.
    pub fn with_group_node_mut<R>(
        &mut self,
        node_id: Uuid,
        f: impl FnOnce(&mut Self) -> R,
    ) -> OpmResult<R> {
        if self.node_attr().uuid() == node_id {
            return Ok(f(self));
        }
        let node_ref = self.graph.node_recursive_mut(node_id)?;
        let Some(group) = node_ref.as_any_mut().downcast_mut::<Self>() else {
            return Err(OpossumError::Other("could not cast to NodeGroup".into()));
        };
        Ok(f(group))
    }

    /// Execute a mutable operation on the optical node identified by `node_id`.
    ///
    /// # Errors
    /// Returns an error if the node cannot be found in the graph.
    pub fn with_node_mut<R>(
        &mut self,
        node_id: Uuid,
        f: impl FnOnce(&mut dyn Analyzable) -> R,
    ) -> OpmResult<R> {
        if self.node_attr().uuid() == node_id {
            return Ok(f(self));
        }
        let node_ref = self.graph.node_recursive_mut(node_id)?;
        Ok(f(&mut **node_ref))
    }

    /// Execute a mutable operation on the `NodeAttr` of the node identified by `node_id`.
    ///
    /// # Errors
    /// Returns an error if the node cannot be found in the graph.
    pub fn with_node_attr_mut<R>(
        &mut self,
        node_id: Uuid,
        f: impl FnOnce(&mut NodeAttr) -> R,
    ) -> OpmResult<R> {
        if self.node_attr().uuid() == node_id {
            return Ok(f(self.node_attr_mut()));
        }
        let node_ref = self.graph.node_recursive_mut(node_id)?;
        Ok(f(node_ref.node_attr_mut()))
    }

    /// Execute a read-only operation with the `NodeAttr` of the node identified by `node_id`.
    ///
    /// # Errors
    /// Returns an error if the node cannot be found in the graph.
    pub fn with_node_attr<R>(&self, node_id: Uuid, f: impl FnOnce(&NodeAttr) -> R) -> OpmResult<R> {
        if self.node_attr().uuid() == node_id {
            return Ok(f(self.node_attr()));
        }
        let (node_ref, _) = self.node_recursive(node_id)?;
        Ok(f(node_ref.node_attr()))
    }

    /// Returns all nodes of this [`NodeGroup`].
    #[must_use]
    pub fn nodes(&self) -> Vec<&OpticRef> {
        self.graph.nodes()
    }

    /// Returns all node connections of this [`NodeGroup`].
    #[must_use]
    pub fn connections(&self) -> Vec<ConnectionInfo> {
        self.graph.connections()
    }

    /// Returns the number of nodes of this [`NodeGroup`].
    #[must_use]
    pub fn nr_of_nodes(&self) -> usize {
        self.graph.node_count()
    }

    /// Connect two optical nodes within this [`NodeGroup`].
    ///
    /// # Errors
    /// Returns an error if ports are already connected, mapped, or form a cycle.
    pub fn connect_nodes(
        &mut self,
        src_id: Uuid,
        src_port: &str,
        target_id: Uuid,
        target_port: &str,
        distance: Length,
    ) -> OpmResult<GraphDelta> {
        if !self
            .graph()
            .port_map(&PortType::Input)
            .assigned_ports_for_node(target_id)
            .is_empty()
        {
            return Err(OpossumError::OpticPort(format!(
                "Cannot connect node, as port '{target_port}' of node {} is already mapped!",
                target_id.as_simple()
            )));
        }
        if !self
            .graph()
            .port_map(&PortType::Output)
            .assigned_ports_for_node(src_id)
            .is_empty()
        {
            return Err(OpossumError::OpticPort(format!(
                "Cannot connect node, as port '{src_port}' of node {} is already mapped!",
                src_id.as_simple()
            )));
        }

        let group_id = self.node_attr().uuid();
        let mut displaced_port_mappings = Vec::new();

        if let Some(ext_name) = self
            .graph()
            .port_map(&PortType::Output)
            .external_port_name(src_id, src_port)
        {
            displaced_port_mappings.push(RemovedPortMapping {
                group_id,
                port_type: PortType::Output,
                external_name: ext_name,
                internal_node_id: src_id,
                internal_port_name: src_port.to_string(),
            });
        }
        if let Some(ext_name) = self
            .graph()
            .port_map(&PortType::Input)
            .external_port_name(target_id, target_port)
        {
            displaced_port_mappings.push(RemovedPortMapping {
                group_id,
                port_type: PortType::Input,
                external_name: ext_name,
                internal_node_id: target_id,
                internal_port_name: target_port.to_string(),
            });
        }

        self.graph
            .connect_nodes(src_id, src_port, target_id, target_port, distance)?;

        let connection = ConnectionInfo {
            src_id,
            src_port: src_port.to_string(),
            target_id,
            target_port: target_port.to_string(),
            distance,
        };

        Ok(GraphDelta::NodesConnected {
            group_id,
            connection,
            displaced_port_mappings,
        })
    }

    /// Disconnect two optical nodes within this [`NodeGroup`].
    ///
    /// # Errors
    /// Returns an error if the node or connection does not exist.
    pub fn disconnect_nodes(&mut self, src_id: Uuid, src_port: &str) -> OpmResult<GraphDelta> {
        let group_id = self.node_attr().uuid();
        let connection = self
            .graph
            .get_outgoing_connection_info_of_node(src_id)
            .into_iter()
            .find(|conn| conn.src_port == src_port)
            .ok_or_else(|| {
                OpossumError::OpticScenery(format!(
                    "source node {src_id} with port <{src_port}> is not connected"
                ))
            })?;

        self.graph.disconnect_nodes(src_id, src_port)?;

        Ok(GraphDelta::NodesDisconnected {
            group_id,
            connection,
        })
    }

    /// Update the distance of an already existing connection.
    ///
    /// # Errors
    /// Returns an error if the connection does not exist.
    pub fn update_connection_distance(
        &mut self,
        src_id: Uuid,
        src_port: &str,
        distance: Length,
    ) -> OpmResult<GraphDelta> {
        let group_id = self.node_attr().uuid();
        let connection = self
            .graph
            .get_outgoing_connection_info_of_node(src_id)
            .into_iter()
            .find(|conn| conn.src_port == src_port)
            .ok_or_else(|| {
                OpossumError::OpticScenery(format!(
                    "source node {src_id} with port <{src_port}> is not connected"
                ))
            })?;

        let old_distance = connection.distance;
        self.graph
            .update_connection_distance(src_id, src_port, distance)?;

        Ok(GraphDelta::ConnectionDistanceChanged {
            group_id,
            src_id,
            src_port: src_port.to_string(),
            old_distance,
            new_distance: distance,
        })
    }

    /// Map an input port of an internal node to an external port of the group.
    ///
    /// # Errors
    /// Returns an error if the external port is already assigned or internal port is invalid.
    pub fn map_input_port(
        &mut self,
        input_node: Uuid,
        internal_name: &str,
        external_name: &str,
    ) -> OpmResult<GraphDelta> {
        let group_id = self.node_attr().uuid();
        self.graph
            .map_port(input_node, &PortType::Input, internal_name, external_name)?;

        Ok(GraphDelta::PortMapped(RemovedPortMapping {
            group_id,
            port_type: PortType::Input,
            external_name: external_name.to_string(),
            internal_node_id: input_node,
            internal_port_name: internal_name.to_string(),
        }))
    }

    /// Map an output port of an internal node to an external port of the group.
    ///
    /// # Errors
    /// Returns an error if the external port is already assigned or internal port is invalid.
    pub fn map_output_port(
        &mut self,
        output_node: Uuid,
        internal_name: &str,
        external_name: &str,
    ) -> OpmResult<GraphDelta> {
        let group_id = self.node_attr().uuid();
        self.graph
            .map_port(output_node, &PortType::Output, internal_name, external_name)?;

        Ok(GraphDelta::PortMapped(RemovedPortMapping {
            group_id,
            port_type: PortType::Output,
            external_name: external_name.to_string(),
            internal_node_id: output_node,
            internal_port_name: internal_name.to_string(),
        }))
    }

    /// Remove a port mapping and return a [`GraphDelta`] for undo support.
    ///
    /// # Errors
    /// Returns an error if the port mapping does not exist.
    pub fn remove_mapped_port(
        &mut self,
        external_name: &str,
        port_type: PortType,
    ) -> OpmResult<GraphDelta> {
        let group_id = self.node_attr().uuid();
        let (target_id, target_port) = self
            .graph
            .port_map(&port_type)
            .get(external_name)
            .cloned()
            .ok_or_else(|| {
                OpossumError::OpticPort(format!(
                    "port mapping '{external_name}' not found for removal"
                ))
            })?;

        self.graph.remove_mapped_port(external_name, port_type);

        Ok(GraphDelta::PortUnmapped(RemovedPortMapping {
            group_id,
            port_type,
            external_name: external_name.to_string(),
            internal_node_id: target_id,
            internal_port_name: target_port,
        }))
    }

    /// Stores the predecessor distance for an external input port during positioning.
    pub fn add_input_port_distance(&mut self, port_name: &str, distance: Length) {
        self.input_port_distances
            .insert(port_name.to_string(), distance);
    }

    /// Returns a mutable reference to the underlying [`OpticGraph`].
    pub const fn graph_mut(&mut self) -> &mut OpticGraph {
        &mut self.graph
    }

    /// Returns a shared reference to the underlying [`OpticGraph`].
    #[must_use]
    pub const fn graph(&self) -> &OpticGraph {
        &self.graph
    }

    /// Generate an [`AnalysisReport`] for this group.
    ///
    /// # Errors
    /// Returns an error if topological sorting or sub-node reporting fails.
    pub fn toplevel_report(&self, analyzer: AnalyzerKind) -> OpmResult<AnalysisReport> {
        let mut analysis_report = AnalysisReport::default();
        analysis_report.add_scenery(self);

        if !self.graph.is_single_tree() {
            analysis_report.add_note(ReportNote::new(
                ReportLevel::Warning,
                "The system contains unconnected sub-trees. Analysis might not be complete.",
            ));
        }
        let sorted = self.graph.topologically_sorted()?;
        for idx in sorted {
            let node_ref = self.graph.node_by_idx(idx)?;
            let uuid = node_ref.uuid();
            if self.graph.is_stale_node(uuid)? {
                analysis_report.add_note(ReportNote::new(
                    ReportLevel::Warning,
                    &format!(
                        "Node '{}' is unconnected and was skipped during analysis.",
                        node_ref.name()
                    ),
                ));
            } else {
                let uuid_str = uuid.as_simple().to_string();
                match node_ref.node_report(&uuid_str, analyzer)? {
                    NodeReportResult::Report(node_report) => {
                        analysis_report.add_node_report(node_report);
                    }
                    NodeReportResult::Incompatible(note) => {
                        analysis_report.add_note(note);
                    }
                    NodeReportResult::None => {}
                }
            }
        }
        Ok(analysis_report)
    }

    /// Returns a reference to the accumulated rays after ghost focus analysis.
    #[must_use]
    pub const fn accumulated_rays(&self) -> &Vec<HashMap<Uuid, Rays>> {
        &self.accumulated_rays
    }

    /// Adds a ray bundle to the accumulated rays at `bounce` level.
    pub fn add_to_accumulated_rays(&mut self, rays: &Rays, bounce: usize) {
        if self.accumulated_rays.len() <= bounce {
            let mut hashed_rays = HashMap::<Uuid, Rays>::new();
            hashed_rays.insert(rays.uuid(), rays.clone());
            self.accumulated_rays.push(hashed_rays);
        } else {
            self.accumulated_rays[bounce].insert(rays.uuid(), rays.clone());
        }
    }

    /// Clears the data stored on all edges in this graph.
    pub fn clear_edges(&mut self) {
        self.graph.clear_edges();
    }

    /// Sets the underlying [`OpticGraph`].
    pub fn set_graph(&mut self, graph: OpticGraph) {
        self.graph = graph;
    }

    /// Find all source ports in the graph recursively.
    ///
    /// # Errors
    /// Returns an error if querying subgraphs fails.
    pub fn find_source_ports(&self) -> OpmResult<Vec<Uuid>> {
        self.graph.find_source_ports()
    }
}

impl OpticNode for NodeGroup {
    fn geometry(&self) -> OpmResult<Option<Geometry>> {
        Ok(None)
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        let ports_to_be_set = self.node_attr.raw_ports();
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

    fn after_deserialization_hook(&mut self) -> OpmResult<()> {
        self.graph.set_is_inverted(self.node_attr.inverted());
        Ok(())
    }

    fn node_report(&self, uuid: &str, analyzer: AnalyzerKind) -> OpmResult<NodeReportResult> {
        let mut group_props = Properties::default();
        for node in self.graph.nodes() {
            let sub_uuid = node.uuid().as_simple().to_string();
            if let NodeReportResult::Report(node_report) = node.node_report(&sub_uuid, analyzer)? {
                let node_name = node.name();
                if !group_props.contains(node_name) {
                    group_props.create(node_name, "", node_report.into())?;
                }
            }
        }
        if group_props.is_empty() {
            Ok(NodeReportResult::None)
        } else {
            Ok(NodeReportResult::Report(NodeReport::new(
                self.node_type(),
                self.name(),
                uuid,
                group_props,
            )))
        }
    }

    fn set_inverted(&mut self, inverted: bool) -> OpmResult<()> {
        self.graph.set_is_inverted(inverted);
        self.node_attr_mut().set_inverted(inverted);
        Ok(())
    }

    fn reset_data(&mut self) {
        for node in self.graph.g.node_weights_mut() {
            node.reset_data();
        }
        self.accumulated_rays = Vec::<HashMap<Uuid, Rays>>::new();
    }

    fn prepare_volume(&mut self, strategy: &dyn PropagationStrategy) -> OpmResult<()> {
        for node in self.graph.g.node_weights_mut() {
            node.prepare_volume(strategy)?;
        }
        Ok(())
    }

    fn update_surfaces(&mut self) -> OpmResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{
            RayTraceConfig,
            energy::{AnalysisEnergy, EnergyConfig},
            raytrace::AnalysisRayTrace,
        },
        core_optics::{OpticNode, node_attr::NodePositioning},
        joule,
        light::{LightResult, Ray, Rays},
        millimeter, nanometer,
        nodes::{Dummy, EnergyMeter, SourcePort, test_helper::helper::*},
        prelude::RayDataSource,
        reporting::Dottable,
        utils::geom_transformation::Isometry,
    };
    use num_traits::Zero;

    #[test]
    fn default() -> OpmResult<()> {
        let node = NodeGroup::default();
        assert_eq!(node.name(), "group");
        assert_eq!(node.node_type(), "group");
        assert!(!node.node_attr().inverted());
        assert!(!node.expand_view()?);
        assert_eq!(node.node_color(), "yellow");
        assert_eq!(node.graph.edge_count(), 0);
        assert_eq!(node.graph.node_count(), 0);
        Ok(())
    }

    #[test]
    fn expand_view_property() -> OpmResult<()> {
        let mut node = NodeGroup::default();
        node.set_expand_view(true)?;
        assert!(node.expand_view()?);
        node.set_expand_view(false)?;
        assert!(!node.expand_view()?);
        Ok(())
    }

    #[test]
    fn new() {
        let node = NodeGroup::new("test");
        assert_eq!(node.name(), "test");
    }

    #[test]
    fn inverted() -> OpmResult<()> {
        test_inverted::<NodeGroup>()
    }

    #[test]
    fn ports() -> OpmResult<()> {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(Dummy::default())?;
        let sn2_i = og.add_node(Dummy::default())?;
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())?;
        assert!(og.ports().names(&PortType::Input).is_empty());
        assert!(og.ports().names(&PortType::Output).is_empty());
        og.map_input_port(sn1_i, "input_1", "input_1")?;
        assert!(
            og.ports()
                .names(&PortType::Input)
                .contains(&("input_1".to_string()))
        );
        og.map_output_port(sn2_i, "output_1", "output_1")?;
        assert!(
            og.ports()
                .names(&PortType::Output)
                .contains(&("output_1".to_string()))
        );
        Ok(())
    }

    #[test]
    fn ports_inverted() -> OpmResult<()> {
        let mut og = NodeGroup::default();
        let sn1_i = og.add_node(Dummy::default())?;
        let sn2_i = og.add_node(Dummy::default())?;
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())?;
        og.map_input_port(sn1_i, "input_1", "input_1")?;
        og.map_output_port(sn2_i, "output_1", "output_1")?;
        og.set_inverted(true)?;
        assert!(
            og.ports()
                .names(&PortType::Output)
                .contains(&("input_1".to_string()))
        );
        assert!(
            og.ports()
                .names(&PortType::Input)
                .contains(&("output_1".to_string()))
        );
        Ok(())
    }

    #[test]
    fn report() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        scenery.add_node(Dummy::default())?;
        let report = scenery.toplevel_report(AnalyzerKind::Energy)?;
        assert!(
            ron::ser::to_string_pretty(&report, ron::ser::PrettyConfig::new().new_line("\n"))
                .is_ok()
        );
        Ok(())
    }

    #[test]
    fn report_empty() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        AnalysisEnergy::analyze(
            &mut scenery,
            LightResult::default(),
            &EnergyConfig::default(),
        )?;
        scenery.toplevel_report(AnalyzerKind::Energy)?;
        Ok(())
    }

    #[test]
    fn analyze_dummy() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let node1 = scenery.add_node(Dummy::default())?;
        let node2 = scenery.add_node(Dummy::default())?;
        scenery.connect_nodes(node1, "output_1", node2, "input_1", Length::zero())?;
        AnalysisEnergy::analyze(
            &mut scenery,
            LightResult::default(),
            &EnergyConfig::default(),
        )?;
        Ok(())
    }

    #[test]
    fn analyze_empty() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        AnalysisEnergy::analyze(
            &mut scenery,
            LightResult::default(),
            &EnergyConfig::default(),
        )?;
        Ok(())
    }

    #[test]
    fn analyze_energy_threshold() -> OpmResult<()> {
        let mut rays = Rays::from(Ray::new_collimated(
            millimeter!(0., 0., 0.),
            nanometer!(1053.0),
            joule!(1.0),
        )?);
        rays.add_ray(Ray::new_collimated(
            millimeter!(0., 0., 0.),
            nanometer!(1053.0),
            joule!(0.1),
        )?);
        let ray_data_builder = RayDataSource::Raw(rays);
        let mut scenery = NodeGroup::default();
        let i_s = scenery.add_node(SourcePort::default())?;

        let mut em = EnergyMeter::default();
        em.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let i_e = scenery.add_node(em)?;
        scenery.connect_nodes(i_s, "output_1", i_e, "input_1", Length::zero())?;
        let mut raytrace_config = RayTraceConfig::default();
        raytrace_config.set_min_energy_per_ray(joule!(0.5))?;
        raytrace_config.map_source(i_s, ray_data_builder.into());
        AnalysisRayTrace::analyze(&mut scenery, LightResult::default(), &raytrace_config)?;
        let uuid = scenery.node(i_e)?.uuid().as_simple().to_string();
        let NodeReportResult::Report(report) = scenery
            .node(i_e)?
            .node_report(&uuid, AnalyzerKind::RayTrace)?
        else {
            panic!("NodeReportResult should be `Report`");
        };
        if let Proptype::Energy(e) = report.properties().get("Energy")? {
            assert_eq!(e, &joule!(1.0));
        } else {
            panic!("could not get energy")
        }
        Ok(())
    }

    #[test]
    fn get_optic_surface_mut() {
        let mut scenery = NodeGroup::default();
        assert!(scenery.get_optic_surface_mut("input_1").is_none())
    }

    #[test]
    fn delete_mapped_node_cleans_up_outer_connections() -> OpmResult<()> {
        let mut outer_group = NodeGroup::new("outer_group");
        let outer_id = outer_group.node_attr().uuid();
        let mut inner_group = NodeGroup::new("inner_group");

        let n_inside = inner_group.add_node(Dummy::new("inside_node"))?;
        inner_group.map_output_port(n_inside, "output_1", "ext_out")?;

        let inner_id = outer_group.add_node(inner_group)?;
        let n_outside = outer_group.add_node(Dummy::new("outside_node"))?;
        outer_group.connect_nodes(inner_id, "ext_out", n_outside, "input_1", Length::zero())?;

        assert_eq!(outer_group.connections().len(), 1);

        let delta = outer_group.delete_node(n_inside)?;
        assert_eq!(outer_group.connections().len(), 0);

        let GraphDelta::NodeDeleted(deletion_delta) = &delta else {
            panic!("Expected GraphDelta::NodeDeleted variant from delete_node");
        };

        assert_eq!(deletion_delta.target_node_id, n_inside);
        assert_eq!(deletion_delta.deleted_nodes.len(), 1);
        assert_eq!(deletion_delta.deleted_nodes[0].node.uuid(), n_inside);
        assert_eq!(deletion_delta.deleted_nodes[0].parent_group_id, inner_id);

        assert_eq!(deletion_delta.removed_port_mappings.len(), 1);
        let port_mapping = &deletion_delta.removed_port_mappings[0];
        assert_eq!(port_mapping.group_id, inner_id);
        assert_eq!(port_mapping.port_type, PortType::Output);
        assert_eq!(port_mapping.external_name, "ext_out");
        assert_eq!(port_mapping.internal_node_id, n_inside);
        assert_eq!(port_mapping.internal_port_name, "output_1");

        assert_eq!(deletion_delta.removed_connections.len(), 1);
        let conn_record = &deletion_delta.removed_connections[0];
        assert_eq!(conn_record.parent_group_id, outer_id);
        assert_eq!(conn_record.connection.src_id, inner_id);
        assert_eq!(conn_record.connection.src_port, "ext_out");
        assert_eq!(conn_record.connection.target_id, n_outside);
        assert_eq!(conn_record.connection.target_port, "input_1");

        outer_group.apply_undo(&delta)?;

        assert!(outer_group.exists(n_inside));
        assert_eq!(outer_group.connections().len(), 1);
        let restored_conn = &outer_group.connections()[0];
        assert_eq!(restored_conn.src_id, inner_id);
        assert_eq!(restored_conn.src_port, "ext_out");
        assert_eq!(restored_conn.target_id, n_outside);
        assert_eq!(restored_conn.target_port, "input_1");

        Ok(())
    }

    #[test]
    fn delete_mapped_node_cleans_up_nested_outer_connections() -> OpmResult<()> {
        let mut top_group = NodeGroup::new("top_group");
        let mut mid_group = NodeGroup::new("mid_group");
        let mut inner_group = NodeGroup::new("inner_group");

        let n_inside = inner_group.add_node(Dummy::new("inside_node"))?;
        inner_group.map_output_port(n_inside, "output_1", "ext_inner")?;

        let inner_id = mid_group.add_node(inner_group)?;
        mid_group.map_output_port(inner_id, "ext_inner", "ext_mid")?;

        let mid_id = top_group.add_node(mid_group)?;
        let n_outside = top_group.add_node(Dummy::new("outside_node"))?;
        top_group.connect_nodes(mid_id, "ext_mid", n_outside, "input_1", Length::zero())?;

        assert_eq!(top_group.connections().len(), 1);

        let delta = top_group.delete_node(n_inside)?;
        assert_eq!(top_group.connections().len(), 0);

        let GraphDelta::NodeDeleted(deletion_delta) = &delta else {
            panic!("Expected GraphDelta::NodeDeleted variant from delete_node");
        };

        assert_eq!(deletion_delta.deleted_nodes.len(), 1);
        assert_eq!(deletion_delta.removed_connections.len(), 1);
        assert_eq!(deletion_delta.removed_port_mappings.len(), 2);

        top_group.apply_undo(&delta)?;

        assert_eq!(top_group.connections().len(), 1);
        assert!(top_group.exists(n_inside));

        Ok(())
    }
}

#[cfg(test)]
mod group_port_mapping_tests {
    use super::*;
    use crate::{meter, nodes::Dummy};

    fn simple_group() -> OpmResult<NodeGroup> {
        let mut group = NodeGroup::new("g");
        let n1 = group.add_node(Dummy::new("n1"))?;
        group.map_input_port(n1, "input_1", "in")?;
        group.set_expand_view(true)?;
        Ok(group)
    }

    fn nested_group() -> OpmResult<NodeGroup> {
        let mut outer = NodeGroup::new("outer");
        let mut inner = NodeGroup::new("inner");
        let node = inner.add_node(Dummy::new("leaf"))?;
        inner.map_input_port(node, "input_1", "in")?;
        inner.set_expand_view(true)?;
        let inner_id = outer.add_node(inner)?;
        outer.map_input_port(inner_id, "in", "in")?;
        outer.set_expand_view(true)?;
        Ok(outer)
    }

    fn deep_nested_group(depth: usize) -> OpmResult<NodeGroup> {
        let mut leaf_group = NodeGroup::new("leaf_group");
        let leaf_node = leaf_group.add_node(Dummy::new("leaf"))?;
        leaf_group.map_input_port(leaf_node, "input_1", "in")?;
        leaf_group.set_expand_view(true)?;

        let mut current = leaf_group;
        for i in 0..depth {
            let mut parent = NodeGroup::new(&format!("g{i}"));
            let id = parent.add_node(current)?;
            parent.map_input_port(id, "in", "in")?;
            parent.set_expand_view(true)?;
            current = parent;
        }
        Ok(current)
    }

    #[test]
    fn mapped_port_simple_group() -> OpmResult<()> {
        let group = simple_group()?;
        let node_id = group.node_attr().uuid().as_simple().to_string();
        let result = group.get_mapped_port_str("in", &node_id)?;
        assert!(result.contains(":input_1"));
        assert!(result.starts_with('i'));
        Ok(())
    }

    #[test]
    fn connecting_already_mapped_port() -> OpmResult<()> {
        let mut group = NodeGroup::new("g");
        let n1 = group.add_node(Dummy::new("n1"))?;
        group.map_input_port(n1, "input_1", "in")?;
        let n2 = group.add_node(Dummy::default())?;
        let result = group.connect_nodes(n2, "output_1", n1, "input_1", meter!(0.));
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn mapped_port_nested_group() -> OpmResult<()> {
        let group = nested_group()?;
        let node_id = group.node_attr().uuid().as_simple().to_string();
        let result = group.get_mapped_port_str("in", &node_id)?;
        assert!(result.contains(":input_1"));
        assert!(result.starts_with('i'));
        Ok(())
    }

    #[test]
    fn mapped_port_deep_nested_groups() -> OpmResult<()> {
        let group = deep_nested_group(5)?;
        let node_id = group.node_attr().uuid().as_simple().to_string();
        let result = group.get_mapped_port_str("in", &node_id)?;
        assert!(result.contains(":input_1"));
        assert!(result.starts_with('i'));
        Ok(())
    }

    #[test]
    fn collapsed_group_returns_external_port() -> OpmResult<()> {
        let mut group = simple_group()?;
        group.set_expand_view(false)?;
        let node_id = group.node_attr().uuid().as_simple().to_string();
        let result = group.get_mapped_port_str("in", &node_id)?;
        assert_eq!(result, format!("{node_id}:in"));
        Ok(())
    }

    #[test]
    fn unmapped_port_returns_error() -> OpmResult<()> {
        let group = simple_group()?;
        let node_id = group.node_attr().uuid().as_simple().to_string();
        let result = group.get_mapped_port_str("does_not_exist", &node_id);
        assert!(result.is_err());
        Ok(())
    }
}
