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
use crate::{
    analyzers::{AnalyzerKind, propagation_strategy::PropagationStrategy},
    core_optics::{
        NodeAttr, NodeAttrExt, OpticNode, OpticPorts, OpticRef, PortType, node_attr::HasNodeAttr,
    },
    error::{OpmResult, OpossumError},
    light::{
        Rays,
        lightdata::{LightData, light_data_builder::LightDataBuilder},
    },
    nodes::{
        NodeRegistration,
        node_group::optic_graph::delta::{GraphDeletionDelta, GraphDelta, RemovedPortMapping},
    },
    properties::{Properties, Proptype},
    reporting::{
        Dottable,
        analysis_report::AnalysisReport,
        node_report::{NodeReport, NodeReportResult},
        report_note::{ReportLevel, ReportNote},
    },
};
pub use optic_graph::{ConnectionInfo, OpticGraph};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, File},
    io::Write,
    path::PathBuf,
    process::Stdio,
};
use uom::si::f64::Length;
use uuid::Uuid;

inventory::submit! {
    NodeRegistration::new::<NodeGroup>("group", "group node containing other nodes or groups")
}
#[derive(OpmNode, Debug, Clone, Serialize, Deserialize)]
#[manual_analyzable]
/// The basic building block of an optical system. It represents a group of other optical
/// nodes ([`OpticNode`]s) arranged in a (sub)graph.
///
/// # Example
///
/// ```rust
/// use opossum_core::prelude::*;
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
///
/// **Note**: The group node does currently ignore all [`Aperture`](crate::apertures::Aperture) definitions on its publicly
/// mapped input and output ports.
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
    /// This is the standard convenience method that returns only the node's [`Uuid`].
    /// It maintains full backward compatibility with existing tests, documentation,
    /// and examples.
    ///
    /// # Errors
    /// Returns an error if the group is set as inverted or the node already exists.
    pub fn add_node<T: Analyzable + Clone + 'static>(&mut self, node: T) -> OpmResult<Uuid> {
        self.add_node_with_delta(node).map(|(node_id, _)| node_id)
    }

    /// Add a given [`OpticNode`] to the (sub-)graph and return its [`Uuid`] along with
    /// a [`GraphDelta`] for transactional undo tracking.
    ///
    /// # Returns
    /// A tuple containing the new node's [`Uuid`] and the corresponding [`GraphDelta::NodeAdded`].
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

        let delta = GraphDelta::NodeAdded { group_id, node_id };

        Ok((node_id, delta))
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
        self.graph.add_node_ref(node)?;

        let delta = GraphDelta::NodeAdded {
            group_id,
            node_id: uuid,
        };

        Ok((uuid, delta))
    }
    /// Delete a node from the graph.
    ///
    /// Deletes a node with the given [`Uuid`] and cascades through all connected edges,
    /// nested nodes (if deleting a group), reference nodes, and port mappings.
    ///
    /// # Returns
    /// A [`GraphDelta::NodeDeleted`] containing all removed entities for undo restoration.
    ///
    /// # Errors
    /// Returns an error if the node does not exist or the graph is inverted.
    pub fn delete_node(&mut self, node_id: Uuid) -> OpmResult<GraphDelta> {
        let group_id = self.node_attr().uuid();
        let deletion_delta = self.graph.delete_node_with_delta(node_id, group_id)?;
        Ok(GraphDelta::NodeDeleted(deletion_delta))
    }
    /// Remove a single node from the graph **without** cascading to reference nodes.
    ///
    /// Forwards to [`OpticGraph::remove_node_no_cascade`]; see there for why relocations
    /// (group / move / convert) use this instead of [`delete_node`](Self::delete_node).
    ///
    /// # Errors
    ///
    /// Returns an error if the group is inverted, or no node with exactly `node_id` exists in the graph.
    pub fn remove_node_no_cascade(&mut self, node_id: Uuid) -> OpmResult<()> {
        self.graph.remove_node_no_cascade(node_id)
    }

    /// Recursively collects the UUIDs of all nodes contained in this graph,
    /// including nodes inside nested group nodes.
    ///
    /// This function traverses the graph hierarchy depth-first and returns
    /// the UUID of every node that is structurally contained within this graph.
    /// If a node is a group, all nodes inside its internal graph are collected
    /// recursively.
    ///
    /// The returned list:
    /// - Includes all directly contained nodes
    /// - Includes all nodes inside nested groups (at any depth)
    /// - Does NOT include the UUID of any parent or owning node outside this graph
    /// - Does NOT perform any deduplication (UUIDs are assumed to be unique by design)
    ///
    /// This is primarily intended for operations where structural containment
    /// matters (e.g., cascading deletions of group nodes).
    ///
    /// # Errors
    ///
    /// Returns an error if acquiring a lock on any contained node fails.
    pub fn collect_all_contained_node_ids_recursive(&self) -> OpmResult<Vec<Uuid>> {
        let mut result = Vec::new();

        for node_ref in self.nodes() {
            let uuid = node_ref.uuid();

            result.push(uuid);

            // If it is a group -> collect recursively
            if let Some(group) = node_ref.as_any().downcast_ref::<Self>() {
                let mut sub_ids = group.collect_all_contained_node_ids_recursive()?;
                result.append(&mut sub_ids);
            }
        }

        Ok(result)
    }
    /// Recursively collects all optical node references contained in this graph,
    /// including nodes inside nested groups at any hierarchy depth.
    ///
    /// The returned list includes:
    /// - Directly contained nodes
    /// - Nodes within nested subgroups
    /// - The nested group nodes themselves
    ///
    /// # Errors
    /// Returns an error if acquiring a lock on any contained node fails.
    pub fn collect_all_nodes_recursive(&self) -> OpmResult<Vec<OpticRef>> {
        let mut result = Vec::new();

        for node_ref in self.nodes() {
            // Include the current node reference
            result.push(node_ref.clone());

            // If the node is a group, recursively collect all of its nested nodes
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
    /// Returns the hierarchy of nodes starting from the given node and walking up
    /// through its parent groups until the root is reached.
    ///
    /// The returned vector contains tuples of `(Uuid, String)` where:
    /// - `Uuid` is the node ID
    /// - `String` is the node's name
    ///
    /// The hierarchy is ordered **bottom-up**, meaning:
    /// - The first element is the provided `node_id`
    /// - Each following element is the parent group
    /// - The last element is the root node of the hierarchy
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The node cannot be resolved via `node_recursive`
    /// - The internal optic reference cannot be locked
    ///
    /// # Notes
    ///
    /// This function performs a recursive traversal using `node_recursive`
    /// to resolve parent nodes until the root node is reached.
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
    /// This function is mainly useful for setting up a [reference node](crate::nodes::NodeReference).
    ///
    /// # Errors
    ///
    /// This function will return [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node(&self, node_id: Uuid) -> OpmResult<OpticRef> {
        if node_id == self.node_attr.uuid() {
            Ok(OpticRef::new(Box::new(self.clone())))
        } else {
            self.graph.node(node_id)
        }
    }
    /// Return `true` if a node with the given [`Uuid`] exists in the graph.
    ///
    /// This function is similar to [`node`](NodeGroup::node()), but it only returns a boolean value.
    #[must_use]
    pub fn exists(&self, node_id: Uuid) -> bool {
        self.node_recursive(node_id).is_ok()
    }
    /// Return a reference to the optical node specified by its [`Uuid`] and the Uuid of the group in which it is contained.
    ///
    /// This function is similar to [`node`](NodeGroup::node()), but it also recursively searches
    /// for the node in the subnodes of the group.
    ///
    /// # Errors
    ///
    /// This function will return [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node_recursive(&self, node_id: Uuid) -> OpmResult<(OpticRef, Uuid)> {
        self.graph.node_recursive(node_id, self.node_attr().uuid())
    }

    /// Execute a read-only operation on the `NodeGroup` identified by `node_id`.
    ///
    /// If `node_id` equals this group's own UUID, the closure is invoked directly with `&self`
    /// (no lock is taken). Otherwise, the node is looked up in the graph, its internal mutex
    /// is locked, and an `&NodeGroup` is passed to the closure. The lock is held only for the
    /// duration of the closure call.
    ///
    /// # Parameters
    /// - `node_id`: UUID of the target optical node.
    /// - `f`: Closure that receives `&NodeGroup` and returns a value of type `R`.
    ///
    /// # Returns
    /// The value produced by `f`, wrapped in `OpmResult<R>`.
    ///
    /// # Errors
    /// Propagates errors from the underlying lookup and locking:
    /// - The node cannot be found in the graph.
    /// - The node is not a group node.
    /// - The mutex is poisoned (e.g., due to a previous panic while locked).
    ///
    /// # Concurrency
    /// A mutex is only acquired when `node_id != self.uuid()`. Avoid performing operations
    /// inside `f` that would attempt to lock the same node again to prevent deadlocks.
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
    /// If `node_id` equals this group's own UUID, the closure is invoked directly with
    /// `&mut self`. Otherwise, the node is looked up recursively in the graph
    /// and an `&mut NodeGroup` is passed to the closure.
    ///
    /// # Parameters
    /// - `node_id`: UUID of the target optical node.
    /// - `f`: Closure that receives `&mut NodeGroup` and returns a value of type `R`.
    ///
    /// # Returns
    /// The value produced by `f`, wrapped in `OpmResult<R>`.
    ///
    /// # Errors
    /// Propagates errors from the underlying lookup:
    /// - The node cannot be found in the graph.
    /// - The node is not a group node.
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
    /// Provides a mutable reference to the node (as `&mut dyn Analyzable`) for the duration of the closure `f`.
    ///
    /// # Parameters
    /// - `node_id`: UUID of the target node (can be any node type, not necessarily a group).
    /// - `f`: Closure that receives `&mut dyn Analyzable` and returns a value of type `R`.
    ///
    /// # Returns
    /// The value produced by `f`, wrapped in `OpmResult<R>`.
    ///
    /// # Errors
    /// Returns an error if the node with `node_id` cannot be found in the graph.
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
    /// If `node_id` equals this group's own UUID, the closure is invoked directly with
    /// `&mut NodeAttr` from `self`. Otherwise, the node is looked up recursively
    /// in the graph and an `&mut NodeAttr` is passed to the closure.
    ///
    /// # Parameters
    /// - `node_id`: UUID of the target node.
    /// - `f`: Closure that receives `&mut NodeAttr` and returns a value of type `R`.
    ///
    /// # Returns
    /// The value produced by `f`, wrapped in `OpmResult<R>`.
    ///
    /// # Errors
    /// Propagates errors from the underlying lookup:
    /// - The node cannot be found in the graph.
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
    /// If `node_id` equals this group's own UUID, the closure is invoked directly with
    /// `&NodeAttr` from `self`. Otherwise, the node is looked up recursively in the graph
    /// and an `&NodeAttr` is passed to the closure.
    ///
    /// # Parameters
    /// - `node_id`: UUID of the target node.
    /// - `f`: Closure that receives `&NodeAttr` and returns a value of type `R`.
    ///
    /// # Returns
    /// The value produced by `f`, wrapped in `OpmResult<R>`.
    ///
    /// # Errors
    /// Propagates errors from the underlying lookup:
    /// - The node cannot be found in the graph.
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
    /// Connects an output port of `src_id` to an input port of `target_id`.
    /// Any port mappings previously assigned to these ports are recorded as displaced.
    ///
    /// # Returns
    /// A [`GraphDelta::NodesConnected`] containing connection metadata and displaced port mappings.
    ///
    /// # Errors
    /// Returns an error if the ports are invalid, already connected, or form a cycle.
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

        // Capture port mappings that will be displaced by this connection
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
    /// # Returns
    /// A [`GraphDelta::NodesDisconnected`] storing the severed connection information.
    ///
    /// # Errors
    /// Returns an error if the node or connection does not exist.
    pub fn disconnect_nodes(&mut self, src_id: Uuid, src_port: &str) -> OpmResult<GraphDelta> {
        let group_id = self.node_attr().uuid();

        // Retrieve existing connection details before removing the edge
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
    /// # Returns
    /// A [`GraphDelta::ConnectionDistanceChanged`] containing previous and updated distances.
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
    /// # Returns
    /// A [`GraphDelta::PortMapped`] with full mapping specifications.
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
    /// # Returns
    /// A [`GraphDelta::PortMapped`] with full mapping specifications.
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
    /// # Returns
    /// A [`GraphDelta::PortUnmapped`] detailing the removed mapping.
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
            self.graph.node_idx_by_uuid(port_info.0).map_or_else(
                || Ok(format!("i{}:{}", port_info.0.as_simple(), port_info.1)),
                |node_idx| self.graph().create_node_edge_str(node_idx, &port_info.1),
            )
        } else {
            Ok(format!("{node_id}:{port_name}"))
        }
    }
    /// Central undo dispatcher: reverses any graph mutation described by a [`GraphDelta`].
    ///
    /// Handles single-node adjustments, connection lifecycles, port mapping changes,
    /// deep cascading deletions, and composite sequences recursively across group hierarchies.
    ///
    /// # Errors
    /// Returns an error if any restoration phase encounters an invalid node ID, port, or graph cycle.
    pub fn apply_undo(&mut self, delta: &GraphDelta) -> OpmResult<()> {
        let current_group_id = self.node_attr().uuid();

        match delta {
            GraphDelta::NodeAdded { group_id, node_id } => {
                // Remove newly added node without cascading
                if *group_id == current_group_id {
                    self.graph.remove_node_no_cascade(*node_id)?;
                } else {
                    self.with_group_node_mut(*group_id, |group| {
                        group.graph_mut().remove_node_no_cascade(*node_id)
                    })??;
                }
            }

            GraphDelta::NodeDeleted(deletion_delta) => {
                // Revert deep deletion across all phases
                self.revert_deletion(deletion_delta)?;
            }

            GraphDelta::NodesConnected {
                group_id,
                connection,
                displaced_port_mappings,
            } => {
                // 1. Sever the connection that was created
                if *group_id == current_group_id {
                    self.graph
                        .disconnect_nodes(connection.src_id, &connection.src_port)?;
                } else {
                    self.with_group_node_mut(*group_id, |group| {
                        group
                            .graph_mut()
                            .disconnect_nodes(connection.src_id, &connection.src_port)
                    })??;
                }

                // 2. Restore any port mappings that were displaced
                for mapping in displaced_port_mappings {
                    if mapping.group_id == current_group_id {
                        self.graph.map_port(
                            mapping.internal_node_id,
                            &mapping.port_type,
                            &mapping.internal_port_name,
                            &mapping.external_name,
                        )?;
                    } else {
                        self.with_group_node_mut(mapping.group_id, |group| {
                            group.graph_mut().map_port(
                                mapping.internal_node_id,
                                &mapping.port_type,
                                &mapping.internal_port_name,
                                &mapping.external_name,
                            )
                        })??;
                    }
                }
            }

            GraphDelta::NodesDisconnected {
                group_id,
                connection,
            } => {
                // Re-establish connection
                if *group_id == current_group_id {
                    self.graph.connect_nodes(
                        connection.src_id,
                        &connection.src_port,
                        connection.target_id,
                        &connection.target_port,
                        connection.distance,
                    )?;
                } else {
                    self.with_group_node_mut(*group_id, |group| {
                        group.graph_mut().connect_nodes(
                            connection.src_id,
                            &connection.src_port,
                            connection.target_id,
                            &connection.target_port,
                            connection.distance,
                        )
                    })??;
                }
            }

            GraphDelta::ConnectionDistanceChanged {
                group_id,
                src_id,
                src_port,
                old_distance,
                ..
            } => {
                // Restore original distance
                if *group_id == current_group_id {
                    self.graph
                        .update_connection_distance(*src_id, src_port, *old_distance)?;
                } else {
                    self.with_group_node_mut(*group_id, |group| {
                        group.graph_mut().update_connection_distance(
                            *src_id,
                            src_port,
                            *old_distance,
                        )
                    })??;
                }
            }

            GraphDelta::PortMapped(mapping) => {
                // Revert mapping by removing external port
                if mapping.group_id == current_group_id {
                    self.graph
                        .remove_mapped_port(&mapping.external_name, mapping.port_type);
                } else {
                    self.with_group_node_mut(mapping.group_id, |group| {
                        group
                            .graph_mut()
                            .remove_mapped_port(&mapping.external_name, mapping.port_type);
                    })?;
                }
            }

            GraphDelta::PortUnmapped(mapping) => {
                // Re-add port mapping
                if mapping.group_id == current_group_id {
                    self.graph.map_port(
                        mapping.internal_node_id,
                        &mapping.port_type,
                        &mapping.internal_port_name,
                        &mapping.external_name,
                    )?;
                } else {
                    self.with_group_node_mut(mapping.group_id, |group| {
                        group.graph_mut().map_port(
                            mapping.internal_node_id,
                            &mapping.port_type,
                            &mapping.internal_port_name,
                            &mapping.external_name,
                        )
                    })??;
                }
            }

            GraphDelta::Composite(deltas) => {
                // Execute sub-deltas in reverse chronological order
                for sub_delta in deltas.iter().rev() {
                    self.apply_undo(sub_delta)?;
                }
            }
        }

        Ok(())
    }

    /// Reverts a multi-entity deletion by re-inserting nodes, port mappings, and edges in phased order.
    fn revert_deletion(&mut self, delta: &GraphDeletionDelta) -> OpmResult<()> {
        let current_group_id = self.node_attr().uuid();

        // Phase 1: Re-insert all nodes into their original parent groups
        for record in &delta.deleted_nodes {
            if record.parent_group_id == current_group_id {
                self.graph.add_node_ref(record.node.clone())?;
            } else {
                self.with_group_node_mut(record.parent_group_id, |group| {
                    group.graph_mut().add_node_ref(record.node.clone())
                })??;
            }
        }

        // Phase 2: Re-resolve references across all hierarchy levels
        self.graph.resolve_all_references()?;

        // Phase 3: Restore exposed port mappings
        for mapping in &delta.removed_port_mappings {
            if mapping.group_id == current_group_id {
                self.graph.map_port(
                    mapping.internal_node_id,
                    &mapping.port_type,
                    &mapping.internal_port_name,
                    &mapping.external_name,
                )?;
            } else {
                self.with_group_node_mut(mapping.group_id, |group| {
                    group.graph_mut().map_port(
                        mapping.internal_node_id,
                        &mapping.port_type,
                        &mapping.internal_port_name,
                        &mapping.external_name,
                    )
                })??;
            }
        }

        // Phase 4: Reconnect edges
        for conn_rec in &delta.removed_connections {
            let conn = &conn_rec.connection;
            if conn_rec.parent_group_id == current_group_id {
                self.graph.connect_nodes(
                    conn.src_id,
                    &conn.src_port,
                    conn.target_id,
                    &conn.target_port,
                    conn.distance,
                )?;
            } else {
                self.with_group_node_mut(conn_rec.parent_group_id, |group| {
                    group.graph_mut().connect_nodes(
                        conn.src_id,
                        &conn.src_port,
                        conn.target_id,
                        &conn.target_port,
                        conn.distance,
                    )
                })??;
            }
        }

        Ok(())
    }
    /// Returns the expansion flag of this [`NodeGroup`].
    ///   
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
    pub const fn graph_mut(&mut self) -> &mut OpticGraph {
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
    /// Returns the dot-file header of this [`NodeGroup`] graph.
    fn add_dot_header(&self, rankdir: &str) -> String {
        use std::fmt::Write;
        let mut dot_string = String::from("digraph {\n\tfontsize = 10;\n");
        let _ = writeln!(dot_string, "\tcompound = true;");
        let _ = writeln!(dot_string, "\trankdir = \"{rankdir}\";");
        let _ = writeln!(dot_string, "\tlabel=\"{}\"", self.node_attr.name());
        let _ = writeln!(dot_string, "\tfontname=\"Courier-monospace\"");
        let _ = writeln!(
            dot_string,
            "\tnode [fontname=\"Courier-monospace\" fontsize = 10]"
        );
        let _ = writeln!(
            dot_string,
            "\tedge [fontname=\"Courier-monospace\" fontsize = 10]\n"
        );
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
            .map_err(|e| {
                OpossumError::Other(format!(
                    "conversion to image failed: {e}. Maybe `graphviz` is not installed."
                ))
            })?;

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
    /// Find all source ports in the graph.
    ///
    /// This function returns a vector of UUIDs identifying all nodes of the type "source port"
    /// in the optical graph.
    ///
    /// # Returns
    /// A vector of [`Uuid`]s representing the source port nodes.
    ///
    /// # Errors
    /// This function will return an error if the resources could not be locked.
    pub fn find_source_ports(&self) -> OpmResult<Vec<Uuid>> {
        self.graph.find_source_ports()
    }
}

impl OpticNode for NodeGroup {
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
    // fn get_optic_surface_mut(&mut self, _surf_name: &str) -> Option<&mut OpticSurface> {
    //     None
    // }
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
        let mut cloned_group = self.clone();
        if self.node_attr.inverted() {
            cloned_group.graph.invert_graph()?;
        }
        let dot_str = if self.expand_view()? {
            cloned_group.to_dot_expanded_view(node_index, name, inverted, rankdir)
        } else {
            Ok(cloned_group.to_dot_collapsed_view(node_index, name, inverted, ports, rankdir))
        };
        // revert the inversion
        if self.node_attr.inverted() {
            cloned_group.graph.invert_graph()?;
        }
        dot_str
    }
    fn node_color(&self) -> &'static str {
        "yellow"
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
        nodes::{Dummy, EnergyMeter, SourcePort, test_helper::test_helper::*},
        prelude::RayDataSource,
        utils::geom_transformation::Isometry,
    };
    use num_traits::Zero;
    #[test]
    fn default() -> OpmResult<()> {
        let node = NodeGroup::default();
        assert_eq!(node.name(), "group");
        assert_eq!(node.node_type(), "group");
        assert_eq!(node.node_attr().inverted(), false);
        assert_eq!(node.expand_view()?, false);
        assert_eq!(node.node_color(), "yellow");
        assert_eq!(node.graph.edge_count(), 0);
        assert_eq!(node.graph.node_count(), 0);
        Ok(())
    }
    #[test]
    fn expand_view_property() -> OpmResult<()> {
        let mut node = NodeGroup::default();
        node.set_expand_view(true)?;
        assert_eq!(node.expand_view()?, true);
        node.set_expand_view(false)?;
        assert_eq!(node.expand_view()?, false);
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
        // How shall we further parse the output?
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
            assert!(false)
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
        // 1. Create top-level (outer) group and internal (inner) group
        let mut outer_group = NodeGroup::new("outer_group");
        let outer_id = outer_group.node_attr().uuid();
        let mut inner_group = NodeGroup::new("inner_group");

        // 2. Add an optical node inside the inner group
        let n_inside = inner_group.add_node(Dummy::new("inside_node"))?;

        // 3. Map the output port of the inside node to an external port of inner_group
        inner_group.map_output_port(n_inside, "output_1", "ext_out")?;

        // 4. Add the inner group and an additional external node to the outer group
        let inner_id = outer_group.add_node(inner_group)?;
        let n_outside = outer_group.add_node(Dummy::new("outside_node"))?;

        // 5. Connect inner_group's mapped output port to the external node's input port
        outer_group.connect_nodes(inner_id, "ext_out", n_outside, "input_1", Length::zero())?;

        // Verify that the connection exists in outer_group before deletion
        assert_eq!(
            outer_group.connections().len(),
            1,
            "Outer group should have exactly 1 connection before node deletion"
        );

        // 6. Delete the inside node from the outer group and capture the delta
        let delta = outer_group.delete_node(n_inside)?;

        // 7. Verify that outer_group cleaned up its connection due to orphaned ports
        assert_eq!(
            outer_group.connections().len(),
            0,
            "Outer group connections should be cleaned up after deleting a mapped node inside a subgroup"
        );

        // 8. Assertions on the deletion delta:
        // Extract inner GraphDeletionDelta from the GraphDelta enum
        let GraphDelta::NodeDeleted(deletion_delta) = &delta else {
            panic!("Expected GraphDelta::NodeDeleted variant from delete_node");
        };

        // Verify target UUID
        assert_eq!(
            deletion_delta.target_node_id, n_inside,
            "Target node ID in delta must match the deleted node UUID"
        );

        // Verify deleted node record contains n_inside and parent_group_id is inner_id
        assert_eq!(
            deletion_delta.deleted_nodes.len(),
            1,
            "Exactly one node record should be captured in delta"
        );
        assert_eq!(
            deletion_delta.deleted_nodes[0].node.uuid(),
            n_inside,
            "Deleted node record UUID must match n_inside"
        );
        assert_eq!(
            deletion_delta.deleted_nodes[0].parent_group_id, inner_id,
            "Parent group of deleted node must be inner_group"
        );

        // Verify removed port mapping inside inner_group
        assert_eq!(
            deletion_delta.removed_port_mappings.len(),
            1,
            "Exactly one removed port mapping should be recorded"
        );
        let port_mapping = &deletion_delta.removed_port_mappings[0];
        assert_eq!(port_mapping.group_id, inner_id);
        assert_eq!(port_mapping.port_type, PortType::Output);
        assert_eq!(port_mapping.external_name, "ext_out");
        assert_eq!(port_mapping.internal_node_id, n_inside);
        assert_eq!(port_mapping.internal_port_name, "output_1");

        // Verify removed connection record belongs to outer_group
        assert_eq!(
            deletion_delta.removed_connections.len(),
            1,
            "Exactly one removed outer connection should be recorded"
        );
        let conn_record = &deletion_delta.removed_connections[0];
        assert_eq!(conn_record.parent_group_id, outer_id);
        assert_eq!(conn_record.connection.src_id, inner_id);
        assert_eq!(conn_record.connection.src_port, "ext_out");
        assert_eq!(conn_record.connection.target_id, n_outside);
        assert_eq!(conn_record.connection.target_port, "input_1");

        // 9. Revert the deletion using the central apply_undo method and verify complete restoration
        outer_group.apply_undo(&delta)?;

        // Verify the node inside inner_group exists again
        assert!(
            outer_group.exists(n_inside),
            "Inside node must exist in outer_group hierarchy after apply_undo"
        );

        // Verify the outer connection is restored
        assert_eq!(
            outer_group.connections().len(),
            1,
            "Outer group connection must be restored after apply_undo"
        );
        let restored_conn = &outer_group.connections()[0];
        assert_eq!(restored_conn.src_id, inner_id);
        assert_eq!(restored_conn.src_port, "ext_out");
        assert_eq!(restored_conn.target_id, n_outside);
        assert_eq!(restored_conn.target_port, "input_1");

        Ok(())
    }

    #[test]
    fn delete_mapped_node_cleans_up_nested_outer_connections() -> OpmResult<()> {
        // 1. Setup a 3-level hierarchy: top_group -> mid_group -> inner_group
        let mut top_group = NodeGroup::new("top_group");
        let mut mid_group = NodeGroup::new("mid_group");
        let mut inner_group = NodeGroup::new("inner_group");

        // 2. Add node inside inner_group
        let n_inside = inner_group.add_node(Dummy::new("inside_node"))?;
        inner_group.map_output_port(n_inside, "output_1", "ext_inner")?;

        // 3. Map inner_group's output port to mid_group
        let inner_id = mid_group.add_node(inner_group)?;
        mid_group.map_output_port(inner_id, "ext_inner", "ext_mid")?;

        // 4. Map mid_group's output port to top_group and connect to an outside node
        let mid_id = top_group.add_node(mid_group)?;
        let n_outside = top_group.add_node(Dummy::new("outside_node"))?;
        top_group.connect_nodes(mid_id, "ext_mid", n_outside, "input_1", Length::zero())?;

        assert_eq!(top_group.connections().len(), 1);

        // 5. Delete the innermost node from the top-level group and capture delta
        let delta = top_group.delete_node(n_inside)?;

        // 6. Assertions: connections at top_group level must be cleaned up
        assert_eq!(
            top_group.connections().len(),
            0,
            "Cascading cleanup failed to remove top-level connection"
        );

        // 7. Verify delta captures mappings and connections across hierarchy levels
        let GraphDelta::NodeDeleted(deletion_delta) = &delta else {
            panic!("Expected GraphDelta::NodeDeleted variant from delete_node");
        };

        assert_eq!(deletion_delta.deleted_nodes.len(), 1);
        assert_eq!(deletion_delta.removed_connections.len(), 1);
        assert_eq!(deletion_delta.removed_port_mappings.len(), 2); // ext_inner and ext_mid

        // 8. Revert and verify full 3-level reconstruction via apply_undo
        top_group.apply_undo(&delta)?;

        assert_eq!(
            top_group.connections().len(),
            1,
            "Top-level connection must be restored after multi-level apply_undo"
        );
        assert!(
            top_group.exists(n_inside),
            "Innermost node must exist again after multi-level apply_undo"
        );

        Ok(())
    }
}

#[cfg(test)]
mod group_port_mapping_tests {
    use super::*;
    use crate::{meter, nodes::Dummy};

    /*
    ============================================================
    Helper
    ============================================================
    */

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

    /*
    ============================================================
    Tests
    ============================================================
    */

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
