#![warn(missing_docs)]
use crate::{
    analyzers::{Analyzable, energy::AnalysisEnergy},
    error::{OpmResult, OpossumError},
    light_flow::LightFlow,
    light_result::LightResult,
    lightdata::LightData,
    optic_node::OpticNode,
    optic_ports::PortType,
    optic_ref::OpticRef,
    optic_scenery_rsc::SceneryResources,
    port_map::PortMap,
    properties::{Proptype, proptype::format_quantity},
};
use log::warn;
use nalgebra::Vector3;
use petgraph::{
    Directed, Direction,
    algo::{connected_components, is_cyclic_directed, toposort},
    graph::{DiGraph, EdgeIndex, Edges, NodeIndex},
    visit::EdgeRef,
};
use serde::{
    Deserialize, Serialize,
    de::{self, MapAccess, Visitor},
    ser::SerializeStruct,
};
use std::fmt::Write as _;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use uom::si::{f64::Length, length::meter};
use uuid::Uuid;
pub type ConnectionInfo = (Uuid, String, Uuid, String, Length);

/// Data structure representing an optical graph
#[derive(Debug, Default, Clone)]
pub struct OpticGraph {
    g: DiGraph<OpticRef, LightFlow>,
    input_port_map: PortMap,
    output_port_map: PortMap,
    is_inverted: bool,
    external_distances: BTreeMap<String, Length>,
    global_confg: Option<Arc<Mutex<SceneryResources>>>,
}
impl OpticGraph {
    /// Add a new optical node to this [`OpticGraph`].
    ///
    /// This function returns a unique node index ([`Uuid`]) of the added node for later referencing (see `connect_nodes`).
    /// **Note**: While constructing the underlying [`OpticRef`] a random, uuid is assigned.
    ///
    /// # Errors
    /// This function returns an error if the graph is set as `inverted` and a node is added. (This could end up in
    /// a weird / undefined behaviour)
    pub fn add_node<T: Analyzable + 'static>(&mut self, node: T) -> OpmResult<Uuid> {
        if self.is_inverted {
            return Err(OpossumError::OpticGroup(
                "cannot add nodes if group is set as inverted".into(),
            ));
        }
        let node_id = node.node_attr().uuid();
        self.g.add_node(OpticRef::new(
            Arc::new(Mutex::new(node)),
            self.global_confg.clone(),
        ));
        Ok(node_id)
    }
    /// Add an [`OpticRef`] to this [`OpticGraph`].
    ///
    /// This function is similar to [`OpticGraph::add_node`] but allows to add an existing [`OpticRef`] to the graph.
    ///
    /// # Errors
    ///
    /// This function will return an error if the graph is set as `inverted` and a node is added. (This could end up in
    /// a weird / undefined behaviour)
    pub fn add_node_ref(&mut self, node: OpticRef) -> OpmResult<NodeIndex> {
        if self.is_inverted {
            return Err(OpossumError::OpticGroup(
                "cannot add nodes if group is set as inverted".into(),
            ));
        }
        Ok(self.g.add_node(node))
    }
    /// Delete a node from this [`OpticGraph`].
    ///
    /// Deletes a node with the given [`Uuid`] from the graph. All edges connected to this node will be removed as well.
    /// This function also deletes all nodes (and sub-nodes) that reference the given node. The function returns a vector
    /// of all deleted node [`Uuid`]s.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the node with the given [`Uuid`] does not exist.
    /// - the graph is set as `inverted`.
    ///
    /// # Panics
    /// This function could theoretically panic if the uuid of the node is not found while looping over all nodes.
    pub fn delete_node(&mut self, node_id: Uuid) -> OpmResult<Vec<Uuid>> {
        if self.is_inverted {
            return Err(OpossumError::OpticGroup(
                "cannot delete nodes if group is set as inverted".into(),
            ));
        }
        let mut nodes_deleted = vec![];
        // delete node and/or references
        while let Some(node_idx) = self.next_node_with_uuid(node_id) {
            // We have to get the uuid of the node, which could be the (initially) given uuid or the uuid of a reference node
            let node_id = self.node_by_idx(node_idx).unwrap().uuid();
            self.g.remove_node(node_idx);
            // Remove possibly no longer valid port mappings
            self.input_port_map.remove_all_from_uuid(node_id);
            self.output_port_map.remove_all_from_uuid(node_id);

            nodes_deleted.push(node_id);
        }
        // now check if subnodes exist and delete recusively
        for node_ref in self.nodes() {
            let mut node = node_ref
                .optical_ref
                .lock()
                .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?;
            if let Ok(group) = node.as_group_mut() {
                let deleted_nodes = group.graph.delete_node(node_id)?;
                nodes_deleted.extend(deleted_nodes);
            }
        }
        if nodes_deleted.is_empty() {
            return Err(OpossumError::OpticScenery(
                "node with given uuid does not exist".into(),
            ));
        }
        Ok(nodes_deleted)
    }
    /// Return the first [`NodeId`] with the given [`Uuid`] in this [`OpticGraph`].
    ///
    /// This also includes reference nodes referring to the given [`Uuid`]. This function returns
    /// `None` if no node with (or referring to) the given [`Uuid`] was found.
    ///
    /// # Panics
    ///
    /// Panics if the mutex lock fails.
    fn next_node_with_uuid(&self, node_id: Uuid) -> Option<NodeIndex> {
        for node_idx in self.g.node_indices() {
            let node_ref = self.node_by_idx(node_idx).unwrap();
            if node_ref.uuid() == node_id {
                return Some(node_idx);
            }
            let node = node_ref.optical_ref.lock().expect("Mutex lock failed");
            let node_attrs = node.node_attr().clone();
            drop(node);
            if node_attrs.node_type() == "reference" {
                let ref_node_props = node_attrs.properties();
                if let Ok(Proptype::Uuid(ref_uuid)) = ref_node_props.get("reference id") {
                    if *ref_uuid == node_id {
                        return Some(node_idx);
                    }
                }
            }
        }
        None
    }
    /// Connect two optical nodes within this [`OpticGraph`].
    ///
    /// This function connects two optical nodes (referenced by their [`NodeIndex`]) with their respective port names and their geometrical distance
    /// (= propagation length) to each other thus extending the network.
    /// # Errors
    ///
    /// This function will return an error if
    ///   - the [`NodeIndex`] of source or target node does not exist in the [`OpticGraph`]
    ///   - a port name of the source or target node does not exist
    ///   - if a node/port combination was already connected earlier
    ///   - the connection of the nodes would form a loop in the network.
    ///   - the given geometric distance between the nodes is not finite.
    ///
    /// # Panics
    /// This function will panic if the mutex lock fails.
    pub fn connect_nodes(
        &mut self,
        src_id: Uuid,
        src_port: &str,
        target_id: Uuid,
        target_port: &str,
        distance: Length,
    ) -> OpmResult<()> {
        if self.is_inverted {
            return Err(OpossumError::OpticGroup(
                "cannot connect nodes if group is set as inverted".into(),
            ));
        }
        let src_node = self.node_idx_by_uuid(src_id).ok_or_else(|| {
            OpossumError::OpticScenery("source node with given id does not exist".into())
        })?;
        let source = self.g.node_weight(src_node).ok_or_else(|| {
            OpossumError::OpticScenery("source node with given id does not exist".into())
        })?;
        if !source
            .optical_ref
            .lock()
            .unwrap()
            .ports()
            .names(&PortType::Output)
            .contains(&src_port.into())
        {
            let src_ports = source
                .optical_ref
                .lock()
                .unwrap()
                .ports()
                .names(&PortType::Output)
                .join(", ");
            return Err(OpossumError::OpticScenery(format!(
                "source node {} does not have an output port {}. Possible values are: {}",
                source.optical_ref.lock().unwrap(),
                src_port,
                src_ports
            )));
        }
        let target_node = self.node_idx_by_uuid(target_id).ok_or_else(|| {
            OpossumError::OpticScenery("target node with given id does not exist".into())
        })?;
        let target = self.g.node_weight(target_node).ok_or_else(|| {
            OpossumError::OpticScenery("target node with given id does not exist".into())
        })?;
        if !target
            .optical_ref
            .lock()
            .unwrap()
            .ports()
            .names(&PortType::Input)
            .contains(&target_port.into())
        {
            let target_ports = target
                .optical_ref
                .lock()
                .unwrap()
                .ports()
                .names(&PortType::Input)
                .join(", ");
            return Err(OpossumError::OpticScenery(format!(
                "target node {} does not have an input port {}. Possible values are: {}",
                target.optical_ref.lock().unwrap(),
                target_port,
                target_ports
            )));
        }
        if self.src_node_port_exists(src_node, src_port) {
            return Err(OpossumError::OpticScenery(format!(
                "src node <{}> with port <{}> is already connected",
                source.optical_ref.lock().unwrap(),
                src_port
            )));
        }
        if self.target_node_port_exists(target_node, target_port) {
            return Err(OpossumError::OpticScenery(format!(
                "target node {} with port <{}> is already connected",
                target.optical_ref.lock().unwrap(),
                target_port
            )));
        }
        let src_name = source.optical_ref.lock().unwrap().name();
        let target_name = target.optical_ref.lock().unwrap().name();
        let light = LightFlow::new(src_port, target_port, distance)?;
        let edge_index = self.g.add_edge(src_node, target_node, light);
        if is_cyclic_directed(&self.g) {
            self.g.remove_edge(edge_index);
            return Err(OpossumError::OpticScenery(format!(
                "connecting nodes <{src_name}> -> <{target_name}> would form a loop"
            )));
        }
        // remove input port mapping, if no loner valid
        self.input_port_map.remove(target_id, target_port);
        // remove output port mapping, if no loner valid
        self.output_port_map.remove(src_id, src_port);
        Ok(())
    }
    /// Disconnect two optical nodes within this [`OpticGraph`].
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
        let src_idx = self.node_idx_by_uuid(src_id).ok_or_else(|| {
            OpossumError::OpticScenery("node with given index does not exist".into())
        })?;
        let edges = self.g.edges_directed(src_idx, Direction::Outgoing);
        let edge_ref = edges
            .into_iter()
            .filter(|idx| idx.weight().src_port() == src_port)
            .last();
        if let Some(edge_ref) = edge_ref {
            self.g.remove_edge(edge_ref.id());
            Ok(())
        } else {
            let node_ref = self.node(src_id)?;
            let node_info = node_ref
                .optical_ref
                .lock()
                .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?;
            Err(OpossumError::OpticScenery(format!(
                "source node {node_info} with port <{src_port}> is not connected"
            )))
        }
    }
    /// Update the distance of an already existing connection.
    ///
    /// # Errors
    ///
    /// This function will return an error if the connection does not exist.
    pub fn update_connection_distance(
        &mut self,
        src_id: Uuid,
        src_port: &str,
        distance: Length,
    ) -> OpmResult<()> {
        let src_idx = self.node_idx_by_uuid(src_id).ok_or_else(|| {
            OpossumError::OpticScenery("node with given index does not exist".into())
        })?;
        let edges = self.g.edges_directed(src_idx, Direction::Outgoing);
        let edge_ref = edges
            .into_iter()
            .filter(|idx| idx.weight().src_port() == src_port)
            .last();
        if let Some(edge_ref) = edge_ref {
            let edge_id = edge_ref.id();
            let edge = self.g.edge_weight_mut(edge_id);
            if let Some(edge) = edge {
                edge.set_distance(distance);
            }
            Ok(())
        } else {
            let node_ref = self.node(src_id)?;
            let node_info = node_ref
                .optical_ref
                .lock()
                .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?;
            Err(OpossumError::OpticScenery(format!(
                "source node {node_info} with port <{src_port}> is not connected"
            )))
        }
    }
    /// Returns a reference to the input port map of this [`OpticGraph`].
    #[must_use]
    pub const fn port_map(&self, port_type: &PortType) -> &PortMap {
        match port_type {
            PortType::Input => &self.input_port_map,
            PortType::Output => &self.output_port_map,
        }
    }
    fn external_nodes(&self, port_type: &PortType) -> Vec<NodeIndex> {
        let edge_direction = match port_type {
            PortType::Input => Direction::Incoming,
            PortType::Output => Direction::Outgoing,
        };
        let mut nodes: Vec<NodeIndex> = Vec::default();
        for node_idx in self.g.node_indices() {
            let edges = self.edges_directed(node_idx, edge_direction).count();
            let ports = self
                .node_by_idx(node_idx)
                .unwrap()
                .optical_ref
                .lock()
                .expect("Mutex lock failed")
                .ports()
                .names(port_type)
                .len();
            if ports != edges {
                nodes.push(node_idx);
            }
        }
        nodes
    }
    /// Map a port of an internal node to an external port of the group.
    ///
    /// In oder to use an [`OpticGraph`] from the outside, internal nodes / ports must be mapped to be visible. The
    /// corresponding `ports` function only returns ports that have been mapped before.
    /// # Errors
    ///
    /// This function will return an error if
    ///   - an external input port name has already been assigned.
    ///   - the `input_node` / `internal_name` does not exist.
    ///   - the specified `input_node` is not an input node of the group (i.e. fully connected to other internal nodes).
    ///   - the `input_node` has an input port with the specified `internal_name` but is already internally connected.
    pub fn map_port(
        &mut self,
        node_id: Uuid,
        port_type: &PortType,
        internal_name: &str,
        external_name: &str,
    ) -> OpmResult<()> {
        let name_type = match port_type {
            PortType::Input => "input_1",
            PortType::Output => "output_1",
        };
        let port_map = match port_type {
            PortType::Input => &self.input_port_map,
            PortType::Output => &self.output_port_map,
        };
        if port_map.contains_external_name(external_name) {
            return Err(OpossumError::OpticGroup(format!(
                "external {name_type} port name already assigned"
            )));
        }
        let Some(node_idx) = self.node_idx_by_uuid(node_id) else {
            return Err(OpossumError::OpticGroup(format!(
                "node with id {node_id} not found"
            )));
        };
        if !self.external_nodes(port_type).contains(&node_idx) {
            return Err(OpossumError::OpticGroup(format!(
                "node to be mapped is not an {name_type} node of the group"
            )));
        }
        let Some(node) = self.g.node_weight(node_idx) else {
            return Err(OpossumError::OpticGroup(format!(
                "node with id {node_id} not found"
            )));
        };
        if !node
            .optical_ref
            .lock()
            .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?
            .ports()
            .names(port_type)
            .contains(&(internal_name.to_string()))
        {
            return Err(OpossumError::OpticGroup(format!(
                "internal {name_type} port name not found"
            )));
        }
        let edge_direction = match port_type {
            PortType::Input => Direction::Incoming,
            PortType::Output => Direction::Outgoing,
        };

        let edge_connected = match port_type {
            PortType::Input => self
                .g
                .edges_directed(node_idx, edge_direction)
                .map(|e| e.weight().target_port())
                .any(|p| p == internal_name),
            PortType::Output => self
                .g
                .edges_directed(node_idx, edge_direction)
                .map(|e| e.weight().src_port())
                .any(|p| p == internal_name),
        };
        if edge_connected {
            return Err(OpossumError::OpticGroup(format!(
                "port of {name_type} node is already internally connected"
            )));
        }
        match port_type {
            PortType::Input => self
                .input_port_map
                .add(external_name, node_id, internal_name)?,
            PortType::Output => self
                .output_port_map
                .add(external_name, node_id, internal_name)?,
        }
        Ok(())
    }
    /// Returns the incoming data of a node in this [`OpticGraph`].
    ///
    /// This function returns the incoming data of a node with the given [`Uuid`]. If the node is an external node, the
    /// incoming data is mapped to the internal node names.
    #[must_use]
    pub fn get_incoming(&self, node_id: Uuid, incoming_data: &LightResult) -> LightResult {
        if self.is_incoming_node(node_id) {
            let portmap = if self.is_inverted {
                self.output_port_map.clone()
            } else {
                self.input_port_map.clone()
            };
            let mut mapped_light_result = LightResult::default();
            // map group-external data and add
            for incoming in incoming_data {
                if let Some(mapping) = portmap.get(incoming.0) {
                    if node_id == mapping.0 {
                        mapped_light_result.insert(mapping.1.clone(), incoming.1.clone());
                    }
                }
            }
            // add group internal data
            for edge in self.incoming_edges(node_id) {
                mapped_light_result.insert(edge.0.clone(), edge.1.clone());
            }
            mapped_light_result
        } else {
            self.incoming_edges(node_id)
        }
    }

    /// Clear the [`LightData`] stored in the edges of this [`OpticGraph`]. Useful for back-
    /// and forth-propagation in ghost focus analysis.
    pub fn clear_edges(&mut self) {
        for edge in self.g.edge_weights_mut() {
            edge.set_data(None);
        }
    }
    /// Return `true` if the node with the given [`Uuid`] is not connected to any other node.
    ///
    /// # Panics
    /// This function will panic if the node with the given [`Uuid`] does not exist.
    #[must_use]
    pub fn is_stale_node(&self, node_id: Uuid) -> bool {
        let idx = self.node_idx_by_uuid(node_id).unwrap();
        let neighbors = self.g.neighbors_undirected(idx);
        neighbors.count() == 0 && !self.input_port_map.contains_node(node_id)
    }
    /// Update reference to global config for each node in this [`OpticGraph`].
    /// This function is needed after deserialization.
    pub fn update_global_config(&mut self, global_conf: &Option<Arc<Mutex<SceneryResources>>>) {
        for node in self.g.node_weights_mut() {
            node.update_global_config(global_conf.clone());
        }
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
    /// Returns [`OpticRef`] with the given [`Uuid`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the node with the given [`Uuid`] does not exist.
    pub fn node(&self, uuid: Uuid) -> OpmResult<OpticRef> {
        self.g
            .node_weights()
            .find(|node| node.uuid() == uuid)
            .cloned()
            .map_or_else(
                || {
                    Err(OpossumError::OpticScenery(
                        "node with given uuid does not exist".into(),
                    ))
                },
                Ok,
            )
    }
    /// Returns a reference to the optical node specified by its [`Uuid`].
    ///
    /// This function is similar to [`OpticGraph::node`] but also checks recursively for
    /// the node in all sub-groups.
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn node_recursive(&self, uuid: Uuid) -> OpmResult<OpticRef> {
        if let Ok(node) = self.node(uuid) {
            Ok(node)
        } else {
            for node_ref in self.g.node_weights() {
                let mut node = node_ref
                    .optical_ref
                    .lock()
                    .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?;

                if let Ok(group) = node.as_group_mut() {
                    if let Ok(node) = group.graph.node_recursive(uuid) {
                        return Ok(node);
                    }
                }
            }
            Err(OpossumError::OpticScenery(
                "node with given uuid does not exist".into(),
            ))
        }
    }
    /// Return a reference to the optical node specified by its node index.
    ///
    /// This function is mainly useful for setting up a reference node.
    ///
    /// # Errors
    ///
    /// This function will return [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node_by_idx(&self, node: NodeIndex) -> OpmResult<OpticRef> {
        let node = self
            .g
            .node_weight(node)
            .ok_or_else(|| OpossumError::OpticScenery("node index does not exist".into()))?;
        Ok(node.clone())
    }
    /// Return the corresponding [`NodeIndex`] from a node with a given [`Uuid`].
    ///
    /// # Panics
    ///
    /// Panics theoretically if the internal [`NodeIndex`] was not found while looping over all nodes.
    #[must_use]
    pub fn idx_by_uuid(&self, uuid: Uuid) -> Option<NodeIndex> {
        self.g
            .node_indices()
            .find(|idx| self.g.node_weight(*idx).unwrap().uuid() == uuid)
    }
    /// Return a mutable reference to the optical node specified by its node index.
    ///
    /// This function is mainly useful for setting up a reference node.
    ///
    /// # Errors
    ///
    /// This function will return [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node_by_idx_mut(&mut self, node: NodeIndex) -> OpmResult<&mut OpticRef> {
        let node = self
            .g
            .node_weight_mut(node)
            .ok_or_else(|| OpossumError::OpticScenery("node index does not exist".into()))?;
        Ok(node)
    }
    /// Return the (internal graph) [`NodeIndex`] of the node with the given [`Uuid`].
    ///
    /// `None` is returned if the node with the given [`Uuid`] does not exist.
    ///
    /// # Panics
    ///
    /// Panics theoretically, if the internal [`NodeIndex`] was not found while looping over all nodes.
    #[must_use]
    pub fn node_idx_by_uuid(&self, uuid: Uuid) -> Option<NodeIndex> {
        self.g
            .node_indices()
            .find(|idx| self.g.node_weight(*idx).unwrap().uuid() == uuid)
    }
    /// Returns all nodes ([`OpticRef`]) of this [`OpticGraph`].
    #[must_use]
    pub fn nodes(&self) -> Vec<&OpticRef> {
        self.g.node_weights().collect()
    }
    /// Returns all node connections of this [`OpticGraph`].
    ///
    /// # Panics
    ///
    /// Panics theoretically, if the internal [`NodeIndex`]es were not found while looping over all edges.
    #[must_use]
    pub fn connections(&self) -> Vec<ConnectionInfo> {
        let mut connections = Vec::<ConnectionInfo>::new();
        for edge_ref in self.g.edge_references() {
            let src_id = self.g.node_weight(edge_ref.source()).unwrap().uuid();
            let target_id = self.g.node_weight(edge_ref.target()).unwrap().uuid();
            let src_port = edge_ref.weight().src_port();
            let target_port = edge_ref.weight().target_port();
            let dist = edge_ref.weight().distance();
            let connection: ConnectionInfo = (
                src_id,
                src_port.to_string(),
                target_id,
                target_port.to_string(),
                *dist,
            );
            connections.push(connection);
        }
        connections
    }
    fn edge_by_idx(&self, idx: EdgeIndex) -> OpmResult<&LightFlow> {
        self.g
            .edge_weight(idx)
            .ok_or_else(|| OpossumError::Other("could not get edge weight".into()))
    }
    /// Returns the topologically sorted of this [`OpticGraph`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn topologically_sorted(&self) -> OpmResult<Vec<NodeIndex>> {
        toposort(&self.g, None)
            .map_err(|_| OpossumError::Analysis("topological sort failed".into()))
    }
    /// Performs an energy flow analysis of this graph.
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn analyze_energy(&mut self, incoming_data: &LightResult) -> OpmResult<LightResult> {
        if self.is_inverted() {
            self.invert_graph()?;
        }
        let g_clone = self.clone();
        if !self.is_single_tree() {
            warn!("group contains unconnected sub-trees. Analysis might not be complete.");
        }
        let sorted = self.topologically_sorted()?;
        let mut light_result = LightResult::default();
        for idx in sorted {
            let node = g_clone.node_by_idx(idx)?.optical_ref;
            let node_id = g_clone.node_by_idx(idx)?.uuid();
            if self.is_stale_node(node_id) {
                warn!(
                    "graph contains stale (completely unconnected) node {}. Skipping.",
                    node.lock()
                        .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?
                );
            } else {
                let incoming_edges = self.get_incoming(node_id, incoming_data);
                let node_name = format!(
                    "{}",
                    node.lock()
                        .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?
                );
                let outgoing_edges = AnalysisEnergy::analyze(
                    &mut *node
                        .lock()
                        .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?,
                    incoming_edges,
                )
                .map_err(|e| {
                    OpossumError::Analysis(format!("analysis of node {node_name} failed: {e}"))
                })?;
                // If node is sink node, rewrite port names according to output mapping
                if self.is_output_node(idx) {
                    let portmap = if self.is_inverted {
                        self.input_port_map.clone()
                    } else {
                        self.output_port_map.clone()
                    };
                    let node_id = self.node_by_idx(idx)?.uuid();
                    let assigned_ports = portmap.assigned_ports_for_node(node_id);
                    for port in assigned_ports {
                        if let Some(light_data) = outgoing_edges.get(&port.1) {
                            light_result.insert(port.0, light_data.clone());
                        }
                    }
                }
                for outgoing_edge in outgoing_edges {
                    self.set_outgoing_edge_data(idx, &outgoing_edge.0, &outgoing_edge.1);
                }
            }
        }
        if self.is_inverted {
            self.invert_graph()?;
        } // revert initial inversion (if necessary)
        Ok(light_result)
    }
    /// Returns the is single tree of this [`OpticGraph`].
    #[must_use]
    pub fn is_single_tree(&self) -> bool {
        connected_components(&self.g) == 1
    }
    /// Returns the number of nodes in this [`OpticGraph`].
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.g.node_count()
    }
    /// Returns the number of connection (edges) in this [`OpticGraph`].
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.g.edge_count()
    }
    fn is_incoming_node(&self, node_id: Uuid) -> bool {
        let nr_of_input_ports = self
            .node(node_id)
            .unwrap()
            .optical_ref
            .lock()
            .expect("Mutex lock failed")
            .ports()
            .ports(&PortType::Input)
            .len();
        let idx = self.node_idx_by_uuid(node_id).unwrap();
        let nr_of_incoming_edges = self.g.edges_directed(idx, Direction::Incoming).count();
        assert!(
            nr_of_incoming_edges <= nr_of_input_ports,
            "# of incoming edges > # of input ports ???"
        );
        nr_of_incoming_edges < nr_of_input_ports
    }
    /// Returns `true` if the node is an output node.
    ///
    /// This function checks if a node with the given [`NodeIndex`] has an unconnected output port.
    ///
    /// # Panics
    ///
    /// Panics if an error occurs while locking the mutex.
    #[must_use]
    pub fn is_output_node(&self, idx: NodeIndex) -> bool {
        let ports = self
            .node_by_idx(idx)
            .unwrap()
            .optical_ref
            .lock()
            .expect("Mutex lock failed")
            .ports();
        let nr_of_output_ports = ports.ports(&PortType::Output).len();
        let nr_of_outgoing_edges = self.g.edges_directed(idx, Direction::Outgoing).count();
        debug_assert!(
            nr_of_outgoing_edges <= nr_of_output_ports,
            "# of outgoing edges > # of output ports ???"
        );
        nr_of_outgoing_edges < nr_of_output_ports
    }
    fn incoming_edges(&self, node_id: Uuid) -> LightResult {
        let node_idx = self.node_idx_by_uuid(node_id).unwrap();
        let edges = self.g.edges_directed(node_idx, Direction::Incoming);
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
    /// Sets the outgoing edge data of this [`OpticGraph`].
    /// Returns true if data has been passed on, false otherwise
    pub fn set_outgoing_edge_data(&mut self, idx: NodeIndex, port: &str, data: &LightData) -> bool {
        let edges = self.g.edges_directed(idx, Direction::Outgoing);
        let edge_ref = edges
            .into_iter()
            .filter(|idx| idx.weight().src_port() == port)
            .last();
        if let Some(edge_ref) = edge_ref {
            let edge_idx = edge_ref.id();
            let light = self.g.edge_weight_mut(edge_idx);
            if let Some(light) = light {
                light.set_data(Some(data.clone()));
            }
            true
        }
        // else outgoing edge not connected -> data dropped
        else {
            false
        }
    }
    fn edges_directed(&self, idx: NodeIndex, dir: Direction) -> Edges<'_, LightFlow, Directed> {
        self.g.edges_directed(idx, dir)
    }
    /// Inverts the [`OpticGraph`].
    ///
    /// This functions changes all directions of node connections and inverts the nodes itself.
    /// # Errors
    ///
    /// This function will return an error if one tries to invert a graph containing a non-invertable node (eg. source).
    pub fn invert_graph(&mut self) -> OpmResult<()> {
        for node in self.g.node_weights_mut() {
            let node_to_be_inverted = !node
                .optical_ref
                .lock()
                .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?
                .inverted();

            node.optical_ref
                .lock()
                .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?
                .set_inverted(node_to_be_inverted)
                .map_err(|_| {
                    OpossumError::OpticGroup(
                        "group cannot be inverted because it contains a non-invertable node".into(),
                    )
                })?;
        }
        for edge in self.g.edge_weights_mut() {
            edge.inverse();
        }
        self.g.reverse();
        Ok(())
    }
    /// Sets the node isometry of this [`OpticGraph`].
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///  - the given `node_id` was not found in the graph.
    ///  - the given `incoming_edges` are not of type `LightData::Geometric`.
    ///  - the given `incoming_edges` contain no rays.
    ///  - the resulting isometry is inconsistent with a previously placed node.
    ///  - the mutex lock failed.
    pub fn set_node_isometry(
        &self,
        incoming_edges: &LightResult,
        node_id: Uuid,
        up_direction: Vector3<f64>,
    ) -> OpmResult<()> {
        for incoming_edge in incoming_edges {
            let node_ref = self.node(node_id)?;
            let distance_from_predecessor =
                self.distance_from_predecessor(node_id, incoming_edge.0)?;
            let mut node = node_ref
                .optical_ref
                .lock()
                .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?;
            if let Ok(group) = node.as_group_mut() {
                group.add_input_port_distance(incoming_edge.0, distance_from_predecessor);
            }
            let LightData::Geometric(rays) = incoming_edge.1 else {
                return Err(OpossumError::Analysis(
                    "expected LightData::Geometric at input port".into(),
                ));
            };
            if let Some(ray) = rays.into_iter().next() {
                let mut ray = ray.to_owned();
                ray.propagate(distance_from_predecessor)?;
                let node_iso = ray.to_isometry(up_direction);
                // if a node with more than one input was already placed (in an earlier loop cycle),
                // check, if the resulting isometry is consistent
                {
                    if let Some(iso) = node.isometry() {
                        if iso != node_iso {
                            warn!("Node {} cannot be consistently positioned.", node.name());
                            warn!("Position based on previous input port is: {iso}");
                            warn!("Position based on this port would be:     {node_iso}");
                            warn!("Keeping first position");
                        }
                    } else {
                        node.set_isometry(node_iso)?;
                        drop(node);
                    }
                }
            } else {
                return Err(OpossumError::Analysis(
                    "no rays in this ray bundle. cannot position nodes".into(),
                ));
            }
        }
        Ok(())
    }
    /// Creates the dot-format string which describes the edge that connects two nodes
    ///
    /// # Parameters:
    /// * `end_node_idx`:         [`NodeIndex`] of the node that should be connected
    /// * `light_port`:           port name that should be connected
    ///
    /// Returns the result of the edge strnig for the dot format
    fn create_node_edge_str(&self, end_node_idx: NodeIndex, light_port: &str) -> OpmResult<String> {
        let node_id = format!("i{}", self.node_by_idx(end_node_idx)?.uuid().as_simple());
        let node_ref = self.node_by_idx(end_node_idx)?;
        let mut node = node_ref
            .optical_ref
            .lock()
            .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?;
        if let Ok(group_node) = node.as_group_mut() {
            Ok(group_node.get_mapped_port_str(light_port, &node_id)?)
        } else {
            Ok(format!("{node_id}:{light_port}"))
        }
    }
    /// Retruns a string of a graphwiz structure of this group.
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn create_dot_string(&self, rankdir: &str) -> OpmResult<String> {
        //check direction
        let rankdir = if rankdir == "LR" { "LR" } else { "TB" };
        let mut dot_string = String::default();
        let sorted = self.topologically_sorted()?;
        for idx in &sorted {
            let node_ref = self.node_by_idx(*idx)?;
            let node = node_ref
                .optical_ref
                .lock()
                .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?;
            let node_name = node.name();
            let inverted = node.inverted();
            let ports = node.ports();
            let uuid = node.node_attr().uuid().as_simple().to_string();
            dot_string += &node.to_dot(&uuid, &node_name, inverted, &ports, rankdir)?;
        }
        for edge_idx in self.g.edge_indices() {
            let light: &LightFlow = self.edge_by_idx(edge_idx)?;
            let end_nodes = self
                .g
                .edge_endpoints(edge_idx)
                .ok_or_else(|| OpossumError::Other("could not get edge_endpoints".into()))?;
            let node_id = self.node_by_idx(end_nodes.1)?.uuid();
            let dist = self.distance_from_predecessor(node_id, light.target_port())?;

            let src_edge_str = self.create_node_edge_str(end_nodes.0, light.src_port())?;
            let target_edge_str = self.create_node_edge_str(end_nodes.1, light.target_port())?;

            let _ = writeln!(
                dot_string,
                "  {src_edge_str} -> {target_edge_str} [label=\"{}\"]",
                format_quantity(meter, dist)
            );
        }
        dot_string.push_str("}\n");
        Ok(dot_string)
    }
    fn distance_from_predecessor(&self, node_id: Uuid, port_name: &str) -> OpmResult<Length> {
        let portmap = if self.is_inverted {
            self.output_port_map.clone()
        } else {
            self.input_port_map.clone()
        };
        if let Some(external_port_name) = portmap.external_port_name(node_id, port_name) {
            self.external_distances.get(&external_port_name).map_or_else(|| Err(OpossumError::Analysis(format!("did not find distance from predecessor to target port '{port_name}' because it's not in the list of external distances"))), |length| Ok(*length))
        } else {
            let idx = self.node_idx_by_uuid(node_id).unwrap();
            let neighbors = self
                .g
                .neighbors_directed(idx, petgraph::Direction::Incoming);
            let mut length = None;
            for neighbor in neighbors {
                let Some(connecting_edge_ref) = self.g.edges_connecting(neighbor, idx).next()
                else {
                    return Err(OpossumError::Analysis(
                        "could not find connecting edge from predecessor".into(),
                    ));
                };
                let connecting_edge = connecting_edge_ref.weight();
                if connecting_edge.target_port() == port_name {
                    length = Some(connecting_edge.distance());
                }
            }
            length.map_or_else(
                || {
                    Err(OpossumError::Analysis(
                        "did not find distance from predecessor to target port".into(),
                    ))
                },
                |length| Ok(*length),
            )
        }
    }
    /// Returns `true` if the graph is inverted.
    #[must_use]
    pub const fn is_inverted(&self) -> bool {
        self.is_inverted
    }
    /// Sets the is inverted of this [`OpticGraph`].
    pub const fn set_is_inverted(&mut self, is_inverted: bool) {
        self.is_inverted = is_inverted;
    }
    /// Sets the external distances of this [`OpticGraph`].
    pub fn set_external_distances(&mut self, external_distances: BTreeMap<String, Length>) {
        self.external_distances = external_distances;
    }
}
impl Serialize for OpticGraph {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut graph = serializer.serialize_struct("graph", 4)?;
        let nodes = self.g.node_weights().cloned().collect::<Vec<OpticRef>>();
        graph.serialize_field("nodes", &nodes)?;
        let edgeidx = self
            .g
            .edge_indices()
            .map(|e| {
                (
                    self.g
                        .node_weight(self.g.edge_endpoints(e).unwrap().0)
                        .unwrap()
                        .uuid(),
                    self.g.edge_weight(e).unwrap().src_port().to_owned(),
                    self.g
                        .node_weight(self.g.edge_endpoints(e).unwrap().1)
                        .unwrap()
                        .uuid(),
                    self.g.edge_weight(e).unwrap().target_port().to_owned(),
                    *self.g.edge_weight(e).unwrap().distance(),
                )
            })
            .collect::<Vec<ConnectionInfo>>();
        graph.serialize_field("edges", &edgeidx)?;
        graph.serialize_field("input_map", &self.input_port_map)?;
        graph.serialize_field("output_map", &self.output_port_map)?;
        graph.end()
    }
}

impl<'de> Deserialize<'de> for OpticGraph {
    #[allow(clippy::too_many_lines)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        enum Field {
            Nodes,
            Edges,
            InputPortMap,
            OutputPortMap,
        }
        const FIELDS: &[&str] = &["nodes", "edges", "input_map", "output_map"];

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct FieldVisitor;

                impl Visitor<'_> for FieldVisitor {
                    type Value = Field;

                    fn expecting(
                        &self,
                        formatter: &mut std::fmt::Formatter<'_>,
                    ) -> std::fmt::Result {
                        formatter.write_str("`nodes`, `edges`, `input_map`, or `output_map`")
                    }
                    fn visit_str<E>(self, value: &str) -> std::result::Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "nodes" => Ok(Field::Nodes),
                            "edges" => Ok(Field::Edges),
                            "input_map" => Ok(Field::InputPortMap),
                            "output_map" => Ok(Field::OutputPortMap),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct OpticGraphVisitor;

        impl<'de> Visitor<'de> for OpticGraphVisitor {
            type Value = OpticGraph;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an OpticGraph")
            }
            fn visit_map<A>(self, mut map: A) -> std::result::Result<OpticGraph, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut g = OpticGraph::default();
                let mut nodes: Option<Vec<OpticRef>> = None;
                let mut edges: Option<Vec<ConnectionInfo>> = None;
                let mut input_map: Option<PortMap> = None;
                let mut output_map: Option<PortMap> = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Nodes => {
                            if nodes.is_some() {
                                return Err(de::Error::duplicate_field("nodes"));
                            }
                            nodes = Some(map.next_value::<Vec<OpticRef>>()?);
                        }
                        Field::Edges => {
                            if edges.is_some() {
                                return Err(de::Error::duplicate_field("edges"));
                            }
                            edges = Some(map.next_value::<Vec<ConnectionInfo>>()?);
                        }
                        Field::InputPortMap => {
                            if input_map.is_some() {
                                return Err(de::Error::duplicate_field("input_map"));
                            }
                            input_map = Some(map.next_value::<PortMap>()?);
                        }
                        Field::OutputPortMap => {
                            if output_map.is_some() {
                                return Err(de::Error::duplicate_field("output_map"));
                            }
                            output_map = Some(map.next_value::<PortMap>()?);
                        }
                    }
                }
                let nodes = nodes.ok_or_else(|| de::Error::missing_field("nodes"))?;
                let edges = edges.ok_or_else(|| de::Error::missing_field("edges"))?;
                for node in &nodes {
                    g.g.add_node(node.clone());
                }
                for node_ref in &nodes {
                    // assign references to ref nodes (if any)
                    assign_reference_to_ref_node(node_ref, &g)
                        .map_err(|e| de::Error::custom(e.to_string()))?;
                }
                for edge in &edges {
                    g.connect_nodes(edge.0, &edge.1, edge.2, &edge.3, edge.4)
                        .map_err(|e| {
                            de::Error::custom(format!("connecting OpticGraph nodes failed: {e}"))
                        })?;
                }
                if let Some(input_map) = input_map {
                    // todo: do sanity check
                    g.input_port_map = input_map;
                }
                if let Some(output_map) = output_map {
                    // todo: do sanity check
                    g.output_port_map = output_map;
                }
                Ok(g)
            }
        }
        deserializer.deserialize_struct("OpticGraph", FIELDS, OpticGraphVisitor)
    }
}

fn assign_reference_to_ref_node(node_ref: &OpticRef, graph: &OpticGraph) -> OpmResult<()> {
    if let Ok(ref_node) = node_ref
        .optical_ref
        .lock()
        .expect("Mutex lock failed")
        .as_refnode_mut()
    {
        // if Ok, the node was indeed a reference node
        let node_props = ref_node.properties().clone();
        let uuid = if let Proptype::Uuid(uuid) = node_props.get("reference id").unwrap() {
            *uuid
        } else {
            Uuid::nil()
        };
        let Ok(reference_node) = graph.node(uuid) else {
            return Err(OpossumError::Other(
                "reference node found, which does not reference anything".into(),
            ));
        };
        let ref_name = format!(
            "ref ({})",
            reference_node
                .optical_ref
                .lock()
                .expect("Mutex lock failed")
                .name()
        );
        ref_node.assign_reference(&reference_node);
        ref_node.node_attr_mut().set_name(&ref_name);
    }
    Ok(())
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        millimeter,
        nodes::{BeamSplitter, Dummy, NodeGroup, NodeReference, Source},
        ray::SplittingConfig,
        spectrum_helper::create_he_ne_spec,
        utils::{geom_transformation::Isometry, test_helper::test_helper::check_logs},
    };
    use approx::assert_abs_diff_eq;
    use num::Zero;
    #[test]
    fn default() {
        let graph = OpticGraph::default();
        assert_eq!(graph.is_inverted, false);
        assert_eq!(graph.g.node_count(), 0)
    }
    #[test]
    fn add_node() {
        let mut og = OpticGraph::default();
        og.add_node(Dummy::default()).unwrap();
        assert_eq!(og.g.node_count(), 1);
    }
    #[test]
    fn add_node_inverted() {
        let mut og = OpticGraph::default();
        og.set_is_inverted(true);
        assert!(og.add_node(Dummy::default()).is_err());
    }
    #[test]
    fn connect_nodes_ok() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        let n2 = graph.add_node(Dummy::default()).unwrap();
        assert!(
            graph
                .connect_nodes(n1, "output_1", n2, "input_1", Length::zero())
                .is_ok()
        );
        assert_eq!(graph.g.edge_count(), 1);
    }
    #[test]
    fn connect_nodes_wrong_ports() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();
        // wrong port names
        let err = og
            .connect_nodes(sn1_i, "wrong", sn2_i, "input_1", Length::zero())
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "OpticScenery:source node 'dummy' (dummy) does not have an output port wrong. Possible values are: output_1"
        );
        assert_eq!(og.g.edge_count(), 0);
        let err = og
            .connect_nodes(sn1_i, "output_1", sn2_i, "wrong", Length::zero())
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "OpticScenery:target node 'dummy' (dummy) does not have an input port wrong. Possible values are: input_1"
        );
        assert_eq!(og.g.edge_count(), 0);
    }
    #[test]
    fn connect_nodes_wrong_index() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        let n2 = graph.add_node(Dummy::default()).unwrap();
        assert!(
            graph
                .connect_nodes(n1, "output_1", Uuid::nil(), "input_1", Length::zero())
                .is_err()
        );
        assert!(
            graph
                .connect_nodes(Uuid::nil(), "output_1", n2, "input_1", Length::zero())
                .is_err()
        );
    }
    #[test]
    fn connect_nodes_wrong_distance() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        let n2 = graph.add_node(Dummy::default()).unwrap();
        assert!(
            graph
                .connect_nodes(n1, "output_1", n2, "input_1", millimeter!(f64::NAN))
                .is_err()
        );
        assert!(
            graph
                .connect_nodes(n1, "output_1", n2, "input_1", millimeter!(f64::INFINITY))
                .is_err()
        );
        assert!(
            graph
                .connect_nodes(
                    n1,
                    "output_1",
                    n2,
                    "input_1",
                    millimeter!(f64::NEG_INFINITY)
                )
                .is_err()
        );
    }
    #[test]
    fn connect_nodes_target_already_connected() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        let n2 = graph.add_node(Dummy::default()).unwrap();
        let n3 = graph.add_node(Dummy::default()).unwrap();
        assert!(
            graph
                .connect_nodes(n1, "output_1", n2, "input_1", Length::zero())
                .is_ok()
        );
        assert!(
            graph
                .connect_nodes(n3, "output_1", n2, "input_1", Length::zero())
                .is_err()
        );
        assert!(
            graph
                .connect_nodes(n1, "output_1", n3, "input_1", Length::zero())
                .is_err()
        );
    }
    #[test]
    fn connect_nodes_loop_error() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        let n2 = graph.add_node(Dummy::default()).unwrap();
        assert!(
            graph
                .connect_nodes(n1, "output_1", n2, "input_1", Length::zero())
                .is_ok()
        );
        assert!(
            graph
                .connect_nodes(n2, "output_1", n1, "input_1", Length::zero())
                .is_err()
        );
        assert_eq!(graph.g.edge_count(), 1);
    }
    #[test]
    fn connect_nodes_inverted() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();
        og.set_is_inverted(true);
        assert!(
            og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
                .is_err()
        );
    }
    #[test]
    fn connect_nodes_update_port_mapping() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();

        og.map_port(sn2_i, &PortType::Input, "input_1", "input_1")
            .unwrap();
        og.map_port(sn1_i, &PortType::Output, "output_1", "output_1")
            .unwrap();
        assert_eq!(og.input_port_map.len(), 1);
        assert_eq!(og.output_port_map.len(), 1);
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
            .unwrap();
        // delete no longer valid port mapping
        assert_eq!(og.input_port_map.len(), 0);
        assert_eq!(og.output_port_map.len(), 0);
    }
    #[test]
    fn map_input_port() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
            .unwrap();
        // wrong port name
        assert!(
            og.map_port(sn1_i, &PortType::Input, "wrong", "input_1")
                .is_err()
        );
        assert_eq!(og.input_port_map.len(), 0);
        // wrong node index
        assert!(
            og.map_port(Uuid::nil(), &PortType::Input, "input_1", "input_1")
                .is_err()
        );
        assert_eq!(og.input_port_map.len(), 0);
        // map output port
        assert!(
            og.map_port(sn2_i, &PortType::Input, "output_1", "input_1")
                .is_err()
        );
        assert_eq!(og.input_port_map.len(), 0);
        // map internal node
        assert!(
            og.map_port(sn2_i, &PortType::Input, "input_1", "input_1")
                .is_err()
        );
        assert_eq!(og.input_port_map.len(), 0);
        // correct usage
        assert!(
            og.map_port(sn1_i, &PortType::Input, "input_1", "input_1")
                .is_ok()
        );
        assert_eq!(og.input_port_map.len(), 1);
    }
    #[test]
    fn map_input_port_half_connected_nodes() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(BeamSplitter::default()).unwrap();
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
            .unwrap();

        // node port already internally connected
        assert!(
            og.map_port(sn2_i, &PortType::Input, "input_1", "bs_input")
                .is_err()
        );

        // correct usage
        assert!(
            og.map_port(sn1_i, &PortType::Input, "input_1", "input_1")
                .is_ok()
        );
        assert!(
            og.map_port(sn2_i, &PortType::Input, "input_2", "bs_input")
                .is_ok()
        );
        assert_eq!(og.input_port_map.len(), 2);
    }
    #[test]
    fn map_output_port() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
            .unwrap();

        // wrong port name
        assert!(
            og.map_port(sn2_i, &PortType::Output, "wrong", "output_1")
                .is_err()
        );
        assert_eq!(og.output_port_map.len(), 0);
        // wrong node index
        assert!(
            og.map_port(Uuid::nil(), &PortType::Output, "output_1", "output_1")
                .is_err()
        );
        assert_eq!(og.output_port_map.len(), 0);
        // map input port
        assert!(
            og.map_port(sn1_i, &PortType::Output, "input_1", "output_1")
                .is_err()
        );
        assert_eq!(og.output_port_map.len(), 0);
        // map internal node
        assert!(
            og.map_port(sn1_i, &PortType::Output, "output_1", "output_1")
                .is_err()
        );
        assert_eq!(og.output_port_map.len(), 0);
        // correct usage
        assert!(
            og.map_port(sn2_i, &PortType::Output, "output_1", "output_1")
                .is_ok()
        );
        assert_eq!(og.output_port_map.len(), 1);
    }
    #[test]
    fn map_output_port_half_connected_nodes() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(BeamSplitter::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();
        og.connect_nodes(sn1_i, "out1_trans1_refl2", sn2_i, "input_1", Length::zero())
            .unwrap();

        // node port already internally connected
        assert!(
            og.map_port(sn1_i, &PortType::Output, "out1_trans1_refl2", "bs_output")
                .is_err()
        );

        // correct usage
        assert!(
            og.map_port(sn1_i, &PortType::Output, "out2_trans2_refl1", "bs_output")
                .is_ok()
        );
        assert!(
            og.map_port(sn2_i, &PortType::Output, "output_1", "output_1")
                .is_ok()
        );
        assert_eq!(og.output_port_map.len(), 2);
    }
    #[test]
    fn input_nodes() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();
        let sub_node3 = BeamSplitter::new("test", &SplittingConfig::Ratio(0.5)).unwrap();
        let sn3_i = og.add_node(sub_node3).unwrap();
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
            .unwrap();
        og.connect_nodes(sn2_i, "output_1", sn3_i, "input_1", Length::zero())
            .unwrap();
        assert_eq!(
            og.external_nodes(&PortType::Input),
            vec![0.into(), 2.into()]
        )
    }
    #[test]
    fn output_nodes() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sub_node1 = BeamSplitter::new("test", &SplittingConfig::Ratio(0.5)).unwrap();
        let sn2_i = og.add_node(sub_node1).unwrap();
        let sn3_i = og.add_node(Dummy::default()).unwrap();
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
            .unwrap();
        og.connect_nodes(sn2_i, "out1_trans1_refl2", sn3_i, "input_1", Length::zero())
            .unwrap();
        assert_eq!(
            og.external_nodes(&PortType::Input),
            vec![0.into(), 1.into()]
        )
    }
    #[test]
    fn node_by_uuid() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        assert!(graph.node(n1).is_ok());
        assert!(graph.node(Uuid::nil()).is_err());
    }
    #[test]
    fn node_id() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        assert!(graph.node_idx_by_uuid(n1).is_some());
        assert!(graph.node_idx_by_uuid(Uuid::nil()).is_none());
    }
    #[test]
    fn is_single_tree() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        let n2 = graph.add_node(Dummy::default()).unwrap();
        let n3 = graph.add_node(Dummy::default()).unwrap();
        let n4 = graph.add_node(Dummy::default()).unwrap();
        graph
            .connect_nodes(n1, "output_1", n2, "input_1", Length::zero())
            .unwrap();
        graph
            .connect_nodes(n3, "output_1", n4, "input_1", Length::zero())
            .unwrap();
        assert_eq!(graph.is_single_tree(), false);
        graph
            .connect_nodes(n2, "output_1", n3, "input_1", Length::zero())
            .unwrap();
        assert_eq!(graph.is_single_tree(), true);
    }
    #[test]
    fn analyze_empty() {
        let mut node = OpticGraph::default();
        let output = node.analyze_energy(&LightResult::default()).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_subtree_warning() {
        let mut graph = OpticGraph::default();
        let mut dummy = Dummy::default();
        dummy.set_isometry(Isometry::identity()).unwrap();
        let d1 = graph.add_node(dummy).unwrap();
        let mut dummy = Dummy::default();
        dummy.set_isometry(Isometry::identity()).unwrap();
        let d2 = graph.add_node(dummy).unwrap();
        let mut dummy = Dummy::default();
        dummy.set_isometry(Isometry::identity()).unwrap();
        let d3 = graph.add_node(dummy).unwrap();
        let mut dummy = Dummy::default();
        dummy.set_isometry(Isometry::identity()).unwrap();
        let d4 = graph.add_node(dummy).unwrap();
        graph
            .connect_nodes(d1, "output_1", d2, "input_1", Length::zero())
            .unwrap();
        graph
            .connect_nodes(d3, "output_1", d4, "input_1", Length::zero())
            .unwrap();
        graph
            .map_port(d1, &PortType::Input, "input_1", "input_1")
            .unwrap();
        let input = LightResult::default();
        testing_logger::setup();
        graph.analyze_energy(&input).unwrap();
        check_logs(
            log::Level::Warn,
            vec!["group contains unconnected sub-trees. Analysis might not be complete."],
        );
    }
    #[test]
    fn analyze_stale_node() {
        let mut graph = OpticGraph::default();
        let mut dummy = Dummy::default();
        dummy.set_isometry(Isometry::identity()).unwrap();
        let d1 = graph.add_node(dummy).unwrap();
        let _ = graph.add_node(Dummy::new("stale node")).unwrap();
        graph
            .map_port(d1, &PortType::Input, "input_1", "input_1")
            .unwrap();
        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Fourier);
        testing_logger::setup();
        assert!(graph.analyze_energy(&input).is_ok());
        check_logs(
            log::Level::Warn,
            vec![
                "group contains unconnected sub-trees. Analysis might not be complete.",
                "graph contains stale (completely unconnected) node 'stale node' (dummy). Skipping.",
            ],
        );
    }
    fn prepare_group() -> OpticGraph {
        let mut graph = OpticGraph::default();
        let g1_n1 = graph.add_node(Dummy::default()).unwrap();
        let g1_n2 = graph
            .add_node(BeamSplitter::new("test", &SplittingConfig::Ratio(0.6)).unwrap())
            .unwrap();
        graph
            .map_port(g1_n2, &PortType::Output, "out1_trans1_refl2", "output_1")
            .unwrap();
        graph
            .map_port(g1_n1, &PortType::Input, "input_1", "input_1")
            .unwrap();
        graph
            .connect_nodes(g1_n1, "output_1", g1_n2, "input_1", Length::zero())
            .unwrap();
        graph
    }
    #[test]
    fn analyze_ok() {
        let mut graph = prepare_group();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("input_1".into(), input_light.clone());
        let output = graph.analyze_energy(&input);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("output_1"));
        let output = output.get("output_1").unwrap().clone();
        let energy = if let LightData::Energy(s) = output {
            s.total_energy()
        } else {
            panic!()
        };
        assert_abs_diff_eq!(energy, 0.6);
    }
    #[test]
    fn analyze_wrong_input_data() {
        let mut graph = prepare_group();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("wrong".into(), input_light.clone());
        let output = graph.analyze_energy(&input).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn analyze_inverse() {
        let mut graph = prepare_group();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        graph.set_is_inverted(true);
        input.insert("output_1".into(), input_light);
        let output = graph.analyze_energy(&input);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("input_1"));
        let output = output.get("input_1").unwrap().clone();
        let energy = if let LightData::Energy(s) = output {
            s.total_energy()
        } else {
            panic!()
        };
        assert_abs_diff_eq!(energy, 0.6);
    }
    #[test]
    fn analyze_inverse_with_src() {
        let mut graph = OpticGraph::default();
        let g1_n1 = graph.add_node(Source::default()).unwrap();
        let g1_n2 = graph.add_node(Dummy::default()).unwrap();
        graph
            .map_port(g1_n2, &PortType::Output, "output_1", "output_1")
            .unwrap();
        graph
            .connect_nodes(g1_n1, "output_1", g1_n2, "input_1", Length::zero())
            .unwrap();
        graph.set_is_inverted(true);
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("output_1".into(), input_light);
        let output = graph.analyze_energy(&input);
        assert!(output.is_err());
    }
    #[test]
    fn serialize_deserialize() {
        let mut graph = OpticGraph::default();
        let i_d1 = graph.add_node(Dummy::default()).unwrap();
        let i_d2 = graph.add_node(Dummy::default()).unwrap();
        graph
            .map_port(i_d1, &PortType::Input, "input_1", "input_1")
            .unwrap();
        graph
            .map_port(i_d2, &PortType::Input, "input_1", "input_2")
            .unwrap();
        let mut port_names = graph.port_map(&PortType::Input).port_names();
        port_names.sort();
        assert_eq!(port_names, vec!["input_1", "input_2"]);
        let serialized =
            ron::ser::to_string_pretty(&graph, ron::ser::PrettyConfig::new().new_line("\n"))
                .unwrap();
        let deserialized: OpticGraph = ron::from_str(&serialized).unwrap();
        let mut port_names = deserialized.port_map(&PortType::Input).port_names();
        port_names.sort();
        assert_eq!(port_names, vec!["input_1", "input_2"]);
    }
    #[test]
    fn next_node_with_uuid_single() {
        let mut graph = OpticGraph::default();
        let i_d1 = graph.add_node(Dummy::default()).unwrap();
        let i_d2 = graph.add_node(Dummy::default()).unwrap();
        let ref_node = NodeReference::from_node(&graph.node(i_d1).unwrap());
        let _ = graph.add_node(ref_node).unwrap();

        assert!(graph.next_node_with_uuid(Uuid::nil()).is_none());
        let mut nodes = vec![];
        while let Some(node_idx) = graph.next_node_with_uuid(i_d2) {
            nodes.push(graph.node_by_idx(node_idx).unwrap().uuid());
            graph.g.remove_node(node_idx);
        }
        assert_eq!(nodes.len(), 1);
        assert!(nodes.contains(&i_d2));
    }
    #[test]
    fn next_node_with_uuid_ref() {
        let mut graph = OpticGraph::default();
        let i_d1 = graph.add_node(Dummy::default()).unwrap();
        let _ = graph.add_node(Dummy::default()).unwrap();
        let ref_node = NodeReference::from_node(&graph.node(i_d1).unwrap());
        let i_ref = graph.add_node(ref_node).unwrap();

        let mut nodes = vec![];
        while let Some(node_idx) = graph.next_node_with_uuid(i_d1) {
            nodes.push(graph.node_by_idx(node_idx).unwrap().uuid());
            graph.g.remove_node(node_idx);
        }
        assert_eq!(nodes.len(), 2);
        assert!(nodes.contains(&i_d1));
        assert!(nodes.contains(&i_ref));
    }
    #[test]
    fn delete_node() {
        let mut graph = OpticGraph::default();
        let i_d1 = graph.add_node(Dummy::default()).unwrap();
        let i_d2 = graph.add_node(Dummy::default()).unwrap();
        let ref_node = NodeReference::from_node(&graph.node(i_d1).unwrap());
        let i_ref = graph.add_node(ref_node).unwrap();
        graph
            .connect_nodes(i_d1, "output_1", i_d2, "input_1", Length::zero())
            .unwrap();
        graph
            .connect_nodes(i_d2, "output_1", i_ref, "input_1", Length::zero())
            .unwrap();
        assert!(graph.delete_node(Uuid::nil()).is_err());
        graph.set_is_inverted(true);
        assert!(graph.delete_node(i_d2).is_err());
        graph.set_is_inverted(false);
        assert_eq!(graph.g.node_count(), 3);
        assert_eq!(graph.g.edge_count(), 2);
        let deleted_nodes = graph.delete_node(i_d2).unwrap();
        assert_eq!(graph.g.node_count(), 2);
        assert_eq!(graph.g.edge_count(), 0);
        assert!(deleted_nodes.contains(&i_d2));
    }
    #[test]
    fn delete_node_with_ref() {
        let mut graph = OpticGraph::default();
        let i_d1 = graph.add_node(Dummy::default()).unwrap();
        let i_d2 = graph.add_node(Dummy::default()).unwrap();
        let ref_node = NodeReference::from_node(&graph.node(i_d1).unwrap());
        let i_ref = graph.add_node(ref_node).unwrap();
        let i_d3 = graph.add_node(Dummy::default()).unwrap();
        graph
            .connect_nodes(i_d1, "output_1", i_d2, "input_1", Length::zero())
            .unwrap();
        graph
            .connect_nodes(i_d2, "output_1", i_ref, "input_1", Length::zero())
            .unwrap();
        graph
            .connect_nodes(i_ref, "output_1", i_d3, "input_1", Length::zero())
            .unwrap();
        graph
            .map_port(i_d1, &PortType::Input, "input_1", "ext_input")
            .unwrap();
        graph
            .map_port(i_d3, &PortType::Output, "output_1", "ext_output")
            .unwrap();
        assert_eq!(graph.g.node_count(), 4);
        assert_eq!(graph.g.edge_count(), 3);
        assert_eq!(graph.input_port_map.len(), 1);
        assert_eq!(graph.output_port_map.len(), 1);
        let deleted_nodes = graph.delete_node(i_d1).unwrap();
        assert_eq!(graph.g.node_count(), 2);
        assert_eq!(graph.g.edge_count(), 0);
        assert_eq!(graph.input_port_map.len(), 0);
        assert_eq!(graph.output_port_map.len(), 1);
        assert!(deleted_nodes.contains(&i_d1));
        assert!(deleted_nodes.contains(&i_ref));
    }
    #[test]
    fn delete_node_with_mapped_ref() {
        let mut graph = OpticGraph::default();
        let i_d1 = graph.add_node(Dummy::default()).unwrap();
        let i_d2 = graph.add_node(Dummy::default()).unwrap();
        let ref_node = NodeReference::from_node(&graph.node(i_d1).unwrap());
        let i_ref = graph.add_node(ref_node).unwrap();
        graph
            .connect_nodes(i_d1, "output_1", i_d2, "input_1", Length::zero())
            .unwrap();
        graph
            .connect_nodes(i_d2, "output_1", i_ref, "input_1", Length::zero())
            .unwrap();
        graph
            .map_port(i_d1, &PortType::Input, "input_1", "ext_input")
            .unwrap();
        graph
            .map_port(i_ref, &PortType::Output, "output_1", "ext_output")
            .unwrap();
        assert_eq!(graph.g.node_count(), 3);
        assert_eq!(graph.g.edge_count(), 2);
        assert_eq!(graph.input_port_map.len(), 1);
        assert_eq!(graph.output_port_map.len(), 1);
        graph.delete_node(i_d1).unwrap();
        assert_eq!(graph.g.node_count(), 1);
        assert_eq!(graph.g.edge_count(), 0);
        assert_eq!(graph.input_port_map.len(), 0);
        assert_eq!(graph.output_port_map.len(), 0);
    }
    #[test]
    fn delete_node_with_subnodes() {
        let mut graph = OpticGraph::default();
        let i_d1 = graph.add_node(Dummy::default()).unwrap();

        let mut group = NodeGroup::default();
        let _i_g_d1 = group.add_node(Dummy::default()).unwrap();
        let ref_node = NodeReference::from_node(&graph.node(i_d1).unwrap());
        let i_ref = group.add_node(ref_node).unwrap();
        let _ = graph.add_node(group).unwrap();

        let deleted_nodes = graph.delete_node(i_d1).unwrap();
        assert_eq!(deleted_nodes.len(), 2);
        assert!(deleted_nodes.contains(&i_d1));
        assert!(deleted_nodes.contains(&i_ref));
    }
    #[test]
    fn disconnect_nodes() {
        let mut graph = OpticGraph::default();
        let i_d1 = graph.add_node(Dummy::default()).unwrap();
        let i_d2 = graph.add_node(Dummy::default()).unwrap();
        graph
            .connect_nodes(i_d1, "output_1", i_d2, "input_1", Length::zero())
            .unwrap();
        assert_eq!(graph.g.edge_count(), 1);
        assert!(graph.disconnect_nodes(Uuid::nil(), "output_1").is_err());
        assert!(graph.disconnect_nodes(i_d1, "wrong").is_err());
        graph.disconnect_nodes(i_d1, "output_1").unwrap();
        assert_eq!(graph.g.edge_count(), 0);
    }
    #[test]
    fn node_recursive_simple() {
        let mut graph = OpticGraph::default();
        let i_d1 = graph.add_node(Dummy::default()).unwrap();
        let i_d2 = graph.add_node(Dummy::default()).unwrap();

        assert_eq!(graph.node_recursive(i_d1).unwrap().uuid(), i_d1);
        assert_eq!(graph.node_recursive(i_d2).unwrap().uuid(), i_d2);
        assert!(graph.node_recursive(uuid::Uuid::nil()).is_err());
    }
    #[test]
    fn node_recursive_nested() {
        let mut graph = OpticGraph::default();
        let i_d = graph.add_node(Dummy::default()).unwrap();
        let mut group = NodeGroup::default();
        let i_g_d1 = group.add_node(Dummy::default()).unwrap();
        let i_g_d2 = group.add_node(Dummy::default()).unwrap();

        let mut group2 = NodeGroup::default();
        let i_g_g2_d = group2.add_node(Dummy::default()).unwrap();

        let i_g_g2 = group.add_node(group2).unwrap();

        let i_g = graph.add_node(group).unwrap();
        assert_eq!(graph.node_recursive(i_d).unwrap().uuid(), i_d);
        assert_eq!(graph.node_recursive(i_g).unwrap().uuid(), i_g);
        assert_eq!(graph.node_recursive(i_g_d1).unwrap().uuid(), i_g_d1);
        assert_eq!(graph.node_recursive(i_g_d2).unwrap().uuid(), i_g_d2);
        assert_eq!(graph.node_recursive(i_g_g2).unwrap().uuid(), i_g_g2);
        assert_eq!(graph.node_recursive(i_g_g2_d).unwrap().uuid(), i_g_g2_d);
    }
}
