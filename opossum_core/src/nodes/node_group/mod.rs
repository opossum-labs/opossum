#![warn(missing_docs)]
//! # Node groups
//!
//! A node group is a special type of optical node that can contain other optical nodes (including other groups) and connections between them. It allows to build up complex optical systems in a hierarchical way. The internal structure of a node group can be hidden or shown in the dot format by setting the `expand view` property of the group node. To use a node group from the outside, internal nodes / ports must be mapped to be visible (see [`map_input_port`](NodeGroup::map_input_port()) & [`map_output_port`](NodeGroup::map_output_port()) functions).
mod analysis_energy;
mod analysis_ghostfocus;
mod analysis_raytrace;
mod optic_graph;
pub mod port_map;
use crate::{
    analyzers::Analyzable,
    core_optics::{
        NodeAttr, OpticNode, OpticPorts, OpticRef, PortType, SceneryResources,
        optic_surface::OpticSurface,
    },
    error::{OpmResult, OpossumError},
    light::{
        Rays,
        lightdata::{LightData, light_data_builder::LightDataBuilder},
    },
    nodes::NodeRegistration,
    properties::{Properties, Proptype},
    reporting::Dottable,
    reporting::{
        analysis_report::AnalysisReport,
        node_report::NodeReport,
        report_note::{ReportLevel, ReportNote},
    },
    utils::LockExt,
};
pub use optic_graph::{ConnectionInfo, OpticGraph};
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

inventory::submit! {
    NodeRegistration::new::<NodeGroup>("group", "group node containing other nodes or groups")
}
#[derive(Debug, Clone, Serialize, Deserialize)]
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
            let node = node_ref.optical_ref.lock_opm()?;
            let uuid = node.node_attr().uuid();

            result.push(uuid);

            // If it is a group -> collect recursively
            if let Ok(group) = node.as_group() {
                let mut sub_ids = group.collect_all_contained_node_ids_recursive()?;
                result.append(&mut sub_ids);
            }
        }

        Ok(result)
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
            group_hierarchy.push((node_id, node_attr.name()));
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

    fn store_node_uuid_in_rays_bundle(&self, node_id: Uuid) -> OpmResult<()> {
        let node_ref = self.graph.node(node_id)?;
        let node = node_ref.optical_ref.lock_opm()?;
        let Ok(node_props) = node.node_attr().get_property("light data") else {
            return Ok(());
        };
        let node_props = node_props.clone();
        drop(node);
        if let Proptype::LightData(Some(LightData::Geometric(rays))) = node_props {
            let mut new_rays = rays;
            new_rays.set_node_origin_uuid(node_id);
            let mut node_ref = node_ref.optical_ref.lock_opm()?;
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
            Ok(OpticRef::new(
                Arc::new(Mutex::new(self.clone())),
                self.global_conf().clone(),
            ))
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
        let arc = self.node_recursive(node_id)?.0.optical_ref;
        let guard = arc.lock_opm()?;
        let group = guard.as_group()?;
        let out = f(group);
        drop(guard);

        Ok(out)
    }
    /// Execute a mutable operation on the `NodeGroup` identified by `node_id`.
    ///
    /// If `node_id` equals this group's own UUID, the closure is invoked directly with
    /// `&mut self` (no lock is taken). Otherwise, the node is looked up in the graph,
    /// its internal mutex is locked, and an `&mut NodeGroup` is passed to the closure.
    /// The lock is held only for the duration of the closure call.
    ///
    /// # Parameters
    /// - `node_id`: UUID of the target optical node.
    /// - `f`: Closure that receives `&mut NodeGroup` and returns a value of type `R`.
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
    /// A mutex is only acquired when `node_id != self.uuid()`. Be careful not to call APIs
    /// within `f` that would attempt to lock the same node again to prevent deadlocks.
    ///
    /// # Panic Safety
    /// If `f` panics while the lock is held, the mutex becomes poisoned; subsequent calls may
    /// fail with a poisoned-lock error.
    pub fn with_group_node_mut<R>(
        &mut self,
        node_id: Uuid,
        f: impl FnOnce(&mut Self) -> R,
    ) -> OpmResult<R> {
        if self.node_attr().uuid() == node_id {
            // direct access to self without lock
            return Ok(f(self));
        }

        let arc = self.node_recursive(node_id)?.0.optical_ref;
        let mut guard = arc.lock_opm()?;

        let group = guard.as_group_mut()?;
        let out = f(group);
        drop(guard);
        Ok(out)
    }

    /// Execute a mutable operation on the optical node identified by `node_id`.
    ///
    /// This method locks the node's internal mutex and provides a mutable reference
    /// to the node (as `&mut dyn Analyzable`) for the duration of the closure `f`.
    ///
    /// # Parameters
    /// - `node_id`: UUID of the target node (can be any node type, not necessarily a group).
    /// - `f`: Closure that receives `&mut dyn Analyzable` and returns a value of type `R`.
    ///
    /// # Returns
    /// The value produced by `f`, wrapped in `OpmResult<R>`.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The node with `node_id` cannot be found in the graph.
    /// - The internal mutex is poisoned or cannot be acquired.
    ///
    /// # Concurrency
    /// The lock is only held for the duration of the closure. Avoid calling
    /// functions inside `f` that would attempt to lock the same node to prevent deadlocks.
    ///
    /// # Panic Safety
    /// If `f` panics while the mutex is held, the mutex may become poisoned;
    /// subsequent calls to `with_node_mut` may fail with a poisoned-lock error.
    pub fn with_node_mut<R>(
        &mut self,
        node_id: Uuid,
        f: impl FnOnce(&mut dyn Analyzable) -> R,
    ) -> OpmResult<R> {
        let (node_ref, _) = self.node_recursive(node_id)?;
        let result = f(&mut *node_ref.optical_ref.lock_opm()?);

        Ok(result)
    }

    /// Execute a mutable operation on the `NodeAttr` of the node identified by `node_id`.
    ///
    /// If `node_id` equals this group's own UUID, the closure is invoked directly with
    /// `&mut NodeAttr` from `self` (no lock is taken). Otherwise, the node is looked up
    /// in the graph, its internal mutex is locked, and an `&mut NodeAttr` is passed to
    /// the closure. The lock is held only for the duration of the closure call.
    ///
    /// # Parameters
    /// - `node_id`: UUID of the target node.
    /// - `f`: Closure that receives `&mut NodeAttr` and returns a value of type `R`.
    ///
    /// # Returns
    /// The value produced by `f`, wrapped in `OpmResult<R>`.
    ///
    /// # Errors
    /// Propagates errors from the underlying lookup and locking:
    /// - The node cannot be found in the graph.
    /// - The mutex is poisoned (e.g., due to a previous panic while locked).
    ///
    /// # Concurrency
    /// A mutex is only acquired when `node_id != self.uuid()`. Be careful not to call APIs
    /// within `f` that would attempt to lock the same node again to prevent deadlocks.
    ///
    /// # Panic Safety
    /// If `f` panics while the lock is held, the mutex becomes poisoned; subsequent calls may
    /// fail with a poisoned-lock error.
    pub fn with_node_attr_mut<R>(
        &mut self,
        node_id: Uuid,
        f: impl FnOnce(&mut NodeAttr) -> R,
    ) -> OpmResult<R> {
        if self.node_attr().uuid() == node_id {
            return Ok(f(self.node_attr_mut()));
        }
        let arc = self.node_recursive(node_id)?.0.optical_ref;
        let mut guard = arc.lock_opm()?;
        let node_attr = guard.node_attr_mut();
        let out = f(node_attr);
        drop(guard);

        Ok(out)
    }

    /// Execute a read-only operation with the `NodeAttr` of the node identified by `node_id`.
    ///
    /// If `node_id` equals this group's own UUID, the closure is invoked directly with
    /// `&NodeAttr` from `self` (no lock is taken). Otherwise, the node is looked up
    /// in the graph, its internal mutex is locked, and an `&NodeAttr` is passed to
    /// the closure. The lock is held only for the duration of the closure call.
    ///
    /// # Parameters
    /// - `node_id`: UUID of the target node.
    /// - `f`: Closure that receives `&NodeAttr` and returns a value of type `R`.
    ///
    /// # Returns
    /// The value produced by `f`, wrapped in `OpmResult<R>`.
    ///
    /// # Errors
    /// Propagates errors from the underlying lookup and locking:
    /// - The node cannot be found in the graph.
    /// - The mutex is poisoned (e.g., due to a previous panic while locked).
    ///
    /// # Concurrency
    /// A mutex is only acquired when `node_id != self.uuid()`. Be careful not to call APIs
    /// within `f` that would attempt to lock the same node again to prevent deadlocks.
    ///
    /// # Panic Safety
    /// If `f` panics while the lock is held, the mutex becomes poisoned; subsequent calls may
    /// fail with a poisoned-lock error.
    pub fn with_node_attr<R>(&self, node_id: Uuid, f: impl FnOnce(&NodeAttr) -> R) -> OpmResult<R> {
        if self.node_attr().uuid() == node_id {
            return Ok(f(self.node_attr()));
        }
        let arc = self.node_recursive(node_id)?.0.optical_ref;
        let guard = arc.lock_opm()?;
        let node_attr = guard.node_attr();
        let out = f(node_attr);
        drop(guard);

        Ok(out)
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
    ///  Connect (already existing) optical nodes within this [`NodeGroup`].
    ///
    /// This function connects two optical nodes (referenced by their [`Uuid`]) with their respective port names
    /// and their geometrical distance (= propagation length) to each other thus extending the optical network.
    /// **Note**: The connection of two internal nodes might affect external port mappings (see [`map_input_port`](NodeGroup::map_input_port())
    /// & [`map_output_port`](NodeGroup::map_output_port()) functions). In this case no longer valid mappings will be deleted.
    ///
    /// # Errors
    /// This function returns an [`OpossumError::OpticScenery`] if
    ///   - the group is set as `inverted`. Connecting subnodes of an inverted group node would result in strange behaviour.
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
        if !self
            .graph()
            .port_map(&PortType::Input)
            .assigned_ports_for_node(target_id)
            .is_empty()
        {
            Err(OpossumError::OpticPort(format!(
                "Cannot connect node, as port '{target_port}' of node {} is already mapped!",
                target_id.as_simple()
            )))
        } else if !self
            .graph()
            .port_map(&PortType::Output)
            .assigned_ports_for_node(src_id)
            .is_empty()
        {
            Err(OpossumError::OpticPort(format!(
                "Cannot connect node, as port '{src_port}' of node {} is already mapped!",
                src_id.as_simple()
            )))
        } else {
            self.graph
                .connect_nodes(src_id, src_port, target_id, target_port, distance)
        }
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
    /// Update the distance of an already existing connection.
    ///
    /// # Errors
    ///
    /// This function will return an error if the connection cannot be found.
    pub fn update_connection_distance(
        &mut self,
        src_id: Uuid,
        src_port: &str,
        distance: Length,
    ) -> OpmResult<()> {
        self.graph
            .update_connection_distance(src_id, src_port, distance)
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

    /// Remove a port mapping
    ///
    /// Returns true if successful
    pub fn remove_mapped_port(&mut self, external_name: &str, port_type: PortType) -> bool {
        self.graph.remove_mapped_port(external_name, port_type)
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
    pub fn toplevel_report(&self) -> OpmResult<AnalysisReport> {
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
                        node_ref.optical_ref.lock_opm()?.name()
                    ),
                ));
            } else {
                let uuid_str = uuid.as_simple().to_string();
                let node_report = node_ref.optical_ref.lock_opm()?.node_report(&uuid_str);
                if let Some(node_report) = node_report {
                    analysis_report.add_node_report(node_report);
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
    fn as_group(&self) -> OpmResult<&NodeGroup> {
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
            if let Ok(node_ref) = node.optical_ref.lock_opm()
                && let Some(node_report) = node_ref.node_report(&sub_uuid)
            {
                let node_name = node_ref.name();
                if !(group_props.contains(&node_name)) {
                    group_props
                        .create(&node_name, "", node_report.into())
                        .unwrap();
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
            if let Ok(mut node) = node.optical_ref.lock_opm() {
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
impl Analyzable for NodeGroup {}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{
            RayTraceConfig,
            energy::{AnalysisEnergy, EnergyConfig},
            raytrace::AnalysisRayTrace,
        },
        core_optics::OpticNode,
        joule,
        light::{LightResult, Ray, Rays},
        millimeter, nanometer,
        nodes::{Dummy, EnergyMeter, SourcePort, test_helper::test_helper::*},
        prelude::RayDataSource,
        utils::{LockExt, geom_transformation::Isometry},
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
        assert!(
            og.ports()
                .names(&PortType::Input)
                .contains(&("input_1".to_string()))
        );
        og.map_output_port(sn2_i, "output_1", "output_1").unwrap();
        assert!(
            og.ports()
                .names(&PortType::Output)
                .contains(&("output_1".to_string()))
        );
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
    }
    #[test]
    fn report() {
        let mut scenery = NodeGroup::default();
        scenery.add_node(Dummy::default()).unwrap();
        let report = scenery.toplevel_report().unwrap();
        assert!(
            ron::ser::to_string_pretty(&report, ron::ser::PrettyConfig::new().new_line("\n"))
                .is_ok()
        );
        // How shall we further parse the output?
    }
    #[test]
    fn report_empty() {
        let mut scenery = NodeGroup::default();
        AnalysisEnergy::analyze(
            &mut scenery,
            LightResult::default(),
            &EnergyConfig::default(),
        )
        .unwrap();
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
        AnalysisEnergy::analyze(
            &mut scenery,
            LightResult::default(),
            &EnergyConfig::default(),
        )
        .unwrap();
    }
    #[test]
    fn analyze_empty() {
        let mut scenery = NodeGroup::default();
        AnalysisEnergy::analyze(
            &mut scenery,
            LightResult::default(),
            &EnergyConfig::default(),
        )
        .unwrap();
    }
    #[test]
    fn analyze_energy_threshold() {
        let mut rays = Rays::from(
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(1.0)).unwrap(),
        );
        rays.add_ray(
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(0.1)).unwrap(),
        );
        let ray_data_builder = RayDataSource::Raw(rays);
        let mut scenery = NodeGroup::default();
        let i_s = scenery.add_node(SourcePort::default()).unwrap();

        let mut em = EnergyMeter::default();
        em.set_isometry(Isometry::identity()).unwrap();
        let i_e = scenery.add_node(em).unwrap();
        scenery
            .connect_nodes(i_s, "output_1", i_e, "input_1", Length::zero())
            .unwrap();
        let mut raytrace_config = RayTraceConfig::default();
        raytrace_config.set_min_energy_per_ray(joule!(0.5)).unwrap();
        raytrace_config.map_source(i_s, ray_data_builder.into());
        AnalysisRayTrace::analyze(&mut scenery, LightResult::default(), &raytrace_config).unwrap();
        let uuid = scenery.node(i_e).unwrap().uuid().as_simple().to_string();
        let report = scenery
            .node(i_e)
            .unwrap()
            .optical_ref
            .lock_opm()
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

#[cfg(test)]
mod group_port_mapping_tests {
    use super::*;
    use crate::{meter, nodes::Dummy};

    /*
    ============================================================
    Helper
    ============================================================
    */

    fn simple_group() -> NodeGroup {
        let mut group = NodeGroup::new("g");
        let n1 = group.add_node(Dummy::new("n1")).unwrap();
        group.map_input_port(n1, "input_1", "in").unwrap();
        group.set_expand_view(true).unwrap();
        group
    }

    fn nested_group() -> NodeGroup {
        let mut outer = NodeGroup::new("outer");
        let mut inner = NodeGroup::new("inner");
        let node = inner.add_node(Dummy::new("leaf")).unwrap();
        inner.map_input_port(node, "input_1", "in").unwrap();
        inner.set_expand_view(true).unwrap();
        let inner_id = outer.add_node(inner).unwrap();
        outer.map_input_port(inner_id, "in", "in").unwrap();
        outer.set_expand_view(true).unwrap();
        outer
    }

    fn deep_nested_group(depth: usize) -> NodeGroup {
        let mut leaf_group = NodeGroup::new("leaf_group");
        let leaf_node = leaf_group.add_node(Dummy::new("leaf")).unwrap();
        leaf_group
            .map_input_port(leaf_node, "input_1", "in")
            .unwrap();
        leaf_group.set_expand_view(true).unwrap();

        let mut current = leaf_group;
        for i in 0..depth {
            let mut parent = NodeGroup::new(&format!("g{i}"));
            let id = parent.add_node(current).unwrap();
            parent.map_input_port(id, "in", "in").unwrap();
            parent.set_expand_view(true).unwrap();
            current = parent;
        }
        current
    }

    /*
    ============================================================
    Tests
    ============================================================
    */

    #[test]
    fn mapped_port_simple_group() {
        let group = simple_group();
        let node_id = group.node_attr().uuid().as_simple().to_string();
        let result = group.get_mapped_port_str("in", &node_id).unwrap();
        assert!(result.contains(":input_1"));
        assert!(result.starts_with('i'));
    }

    #[test]
    fn connecting_already_mapped_port() {
        let mut group = NodeGroup::new("g");
        let n1 = group.add_node(Dummy::new("n1")).unwrap();
        group.map_input_port(n1, "input_1", "in").unwrap();
        let n2 = group.add_node(Dummy::default()).unwrap();
        let result = group.connect_nodes(n2, "output_1", n1, "input_1", meter!(0.));
        assert!(result.is_err());
    }

    #[test]
    fn mapped_port_nested_group() {
        let group = nested_group();
        let node_id = group.node_attr().uuid().as_simple().to_string();
        let result = group.get_mapped_port_str("in", &node_id).unwrap();
        assert!(result.contains(":input_1"));
        assert!(result.starts_with('i'));
    }

    #[test]
    fn mapped_port_deep_nested_groups() {
        let group = deep_nested_group(5);
        let node_id = group.node_attr().uuid().as_simple().to_string();
        let result = group.get_mapped_port_str("in", &node_id).unwrap();
        assert!(result.contains(":input_1"));
        assert!(result.starts_with('i'));
    }

    #[test]
    fn collapsed_group_returns_external_port() {
        let mut group = simple_group();
        group.set_expand_view(false).unwrap();
        let node_id = group.node_attr().uuid().as_simple().to_string();
        let result = group.get_mapped_port_str("in", &node_id).unwrap();
        assert_eq!(result, format!("{node_id}:in"));
    }

    #[test]
    fn unmapped_port_returns_error() {
        let group = simple_group();
        let node_id = group.node_attr().uuid().as_simple().to_string();
        let result = group.get_mapped_port_str("does_not_exist", &node_id);
        assert!(result.is_err());
    }
}
