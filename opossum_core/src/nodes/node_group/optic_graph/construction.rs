use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use super::graph::OpticGraph;
use crate::{
    analyzers::Analyzable,
    core_optics::{NodeAttrExt, OpticRef},
    error::{OpmResult, OpossumError},
    light::LightFlow,
    nodes::NodeGroup,
    prelude::PortType,
    properties::Proptype,
    utils::LockExt,
};
use petgraph::{
    Directed, Direction,
    algo::is_cyclic_directed,
    graph::{EdgeIndex, Edges, NodeIndex},
    visit::EdgeRef,
};
use uom::si::f64::Length;
use uuid::Uuid;

impl OpticGraph {
    /// Add a new optical node to this [`OpticGraph`].
    ///
    /// This function returns a unique node index ([`Uuid`]) of the added node for later referencing (see `connect_nodes`).
    /// **Note**: While constructing the underlying `OpticRef` a random `UUID` is assigned.
    ///
    /// # Errors
    ///
    /// This function returns an error if:
    /// - The graph is set as `inverted`.
    /// - A node with the same [`Uuid`] already exists in the graph.
    pub fn add_node<T: Analyzable + 'static>(&mut self, node: T) -> OpmResult<Uuid> {
        if self.is_inverted() {
            return Err(OpossumError::OpticGroup(
                "cannot add nodes if group is set as inverted".into(),
            ));
        }
        let node_id = node.node_attr().uuid();
        // Ensure UUID does not already exist in the graph
        if self.node_idx_by_uuid(node_id).is_some() {
            return Err(OpossumError::OpticGroup(format!(
                "node with uuid {node_id} already exists"
            )));
        }
        self.g.add_node(OpticRef::new(
            Arc::new(Mutex::new(node)),
            self.global_confg(),
        ));
        Ok(node_id)
    }

    /// Add a node to this [`OpticGraph`].
    ///
    /// This function is similar to [`OpticGraph::add_node`] but allows adding an existing `OpticRef` to the graph.
    ///
    /// # Errors
    ///
    /// This function will return an error if the graph is set as `inverted` and a node is added.
    pub fn add_node_ref(&mut self, node: OpticRef) -> OpmResult<NodeIndex> {
        if self.is_inverted() {
            return Err(OpossumError::OpticGroup(
                "cannot add nodes if group is set as inverted".into(),
            ));
        }
        Ok(self.g.add_node(node))
    }

    /// Recursively cleans up connections (edges) and port mappings that refer to
    /// ports that no longer exist on their target or source nodes.
    fn cleanup_orphan_connections_and_mappings(&mut self) -> OpmResult<()> {
        // 1. Recursively clean up sub-groups first so their `ports()` are up-to-date
        for node_ref in self.nodes() {
            let mut node = node_ref.optical_ref.lock_opm()?;
            if let Some(group) = node.as_any_mut().downcast_mut::<NodeGroup>() {
                group.graph.cleanup_orphan_connections_and_mappings()?;
            }
        }

        // 2. Clean up input port mappings pointing to invalid ports or missing nodes
        let mut input_mappings_to_remove = Vec::new();
        for (ext_name, (target_id, target_port)) in &self.input_port_map {
            if let Ok(target_ref) = self.node(*target_id) {
                let valid_ports = target_ref
                    .optical_ref
                    .lock_opm()?
                    .ports()
                    .names(&PortType::Input);
                if !valid_ports.contains(target_port) {
                    input_mappings_to_remove.push(ext_name.clone());
                }
            } else {
                input_mappings_to_remove.push(ext_name.clone());
            }
        }
        for key in input_mappings_to_remove {
            self.input_port_map.remove_key(&key);
        }

        // 3. Clean up output port mappings pointing to invalid ports or missing nodes
        let mut output_mappings_to_remove = Vec::new();
        for (ext_name, (src_id, src_port)) in &self.output_port_map {
            if let Ok(src_ref) = self.node(*src_id) {
                let valid_ports = src_ref
                    .optical_ref
                    .lock_opm()?
                    .ports()
                    .names(&PortType::Output);
                if !valid_ports.contains(src_port) {
                    output_mappings_to_remove.push(ext_name.clone());
                }
            } else {
                output_mappings_to_remove.push(ext_name.clone());
            }
        }
        for key in output_mappings_to_remove {
            self.output_port_map.remove_key(&key);
        }

        // 4. Clean up edges in this graph where source or target port is no longer valid
        let mut edges_to_remove = Vec::new();
        for edge_ref in self.g.edge_references() {
            let src_node_ref = &self.g[edge_ref.source()];
            let target_node_ref = &self.g[edge_ref.target()];

            let src_port = edge_ref.weight().src_port();
            let target_port = edge_ref.weight().target_port();

            let src_valid = {
                let src_guard = src_node_ref.optical_ref.lock_opm()?;
                src_guard
                    .ports()
                    .names(&PortType::Output)
                    .iter()
                    .any(|p| p == src_port)
            };

            let target_valid = {
                let target_guard = target_node_ref.optical_ref.lock_opm()?;
                target_guard
                    .ports()
                    .names(&PortType::Input)
                    .iter()
                    .any(|p| p == target_port)
            };

            if !src_valid || !target_valid {
                edges_to_remove.push(edge_ref.id());
            }
        }

        for edge_idx in edges_to_remove {
            self.g.remove_edge(edge_idx);
        }

        Ok(())
    }

    /// Delete a node from this [`OpticGraph`].
    ///
    /// Deletes a node with the given [`Uuid`] from the graph. All edges connected to this node will be removed as well.
    /// This function also deletes all nodes (and sub-nodes) that reference the given node. It also deletes possible cascades
    /// of reference nodes (reference nodes of reference nodes referring to the given uuid). The function returns a vector
    /// of all deleted node [`Uuid`]s.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the node with the given [`Uuid`] does not exist.
    /// - the graph is set as `inverted`.
    pub fn delete_node(&mut self, node_id: Uuid) -> OpmResult<Vec<Uuid>> {
        if self.is_inverted() {
            return Err(OpossumError::OpticGroup(
                "cannot delete nodes if group is set as inverted".into(),
            ));
        }
        let mut nodes_deleted = vec![];
        let mut deletion_queue = vec![node_id];
        let mut processed_uuids = HashSet::new();

        while let Some(current_id_to_check) = deletion_queue.pop() {
            if !processed_uuids.insert(current_id_to_check) {
                continue;
            }
            while let Some(node_idx) = self.find_first_node_with_uuid(current_id_to_check) {
                // Avoid redundant node lookup by caching node_ref
                let node_ref = self.node_by_idx(node_idx)?;
                let actual_node_id = node_ref.uuid()?;

                {
                    let node = node_ref.optical_ref.lock_opm()?;
                    if let Some(group) = node.as_any().downcast_ref::<NodeGroup>()
                        && let Ok(sub_ids) = group.collect_all_contained_node_ids_recursive()
                    {
                        for id in sub_ids {
                            deletion_queue.push(id);
                            nodes_deleted.push(id);
                        }
                    }
                }

                self.g.remove_node(node_idx);
                self.input_port_map.remove_all_from_uuid(actual_node_id);
                self.output_port_map.remove_all_from_uuid(actual_node_id);

                if !nodes_deleted.contains(&actual_node_id) {
                    nodes_deleted.push(actual_node_id);
                }
                deletion_queue.push(actual_node_id);
            }
        }

        for node_ref in self.nodes() {
            let mut node = node_ref.optical_ref.lock_opm()?;
            if let Some(group) = node.as_any_mut().downcast_mut::<NodeGroup>()
                && let Ok(deleted_nodes_in_group) = group.graph.delete_node(node_id)
            {
                nodes_deleted.extend(deleted_nodes_in_group);
            }
        }
        if nodes_deleted.is_empty() {
            return Err(OpossumError::OpticScenery(
                "node with given uuid does not exist".into(),
            ));
        }

        nodes_deleted.sort();
        nodes_deleted.dedup();

        // Perform cascading cleanup of orphaned connections and port mappings
        self.cleanup_orphan_connections_and_mappings()?;

        Ok(nodes_deleted)
    }

    /// Return the first [`NodeIndex`] with the given [`Uuid`] in this [`OpticGraph`].
    ///
    /// This also includes reference nodes referring to the given [`Uuid`]. This function returns
    /// `None` if no node with (or referring to) the given [`Uuid`] was found.
    fn find_first_node_with_uuid(&self, node_id: Uuid) -> Option<NodeIndex> {
        for node_idx in self.g.node_indices() {
            let node_ref = self.node_by_idx(node_idx).ok()?;
            if node_ref.uuid().ok()? == node_id {
                return Some(node_idx);
            }
            let node = node_ref.optical_ref.lock_opm().ok()?;
            let node_attrs = node.node_attr().clone();
            drop(node);
            if node_attrs.node_type() == "reference" {
                let ref_node_props = node_attrs.properties();
                if let Ok(Proptype::Uuid(ref_uuid)) = ref_node_props.get("reference id")
                    && *ref_uuid == node_id
                {
                    return Some(node_idx);
                }
            }
        }
        None
    }

    /// Return a map of [`Uuid`]s of nodes that reference a certain node with the given [`Uuid`].
    ///
    /// # Arguments
    ///
    /// * `node_id` - The UUID of the target node to find references to.
    /// * `group_id` - The UUID of the current group context.
    ///
    /// # Errors
    ///
    /// Returns an [`OpmResult::Err`] if:
    /// - Accessing a node by index fails
    /// - Locking a node's `OpticRef` fails
    /// - Any recursive group traversal fails
    pub fn find_all_nodes_referring_to_uuid(
        &self,
        node_id: Uuid,
        group_id: Uuid,
    ) -> OpmResult<HashMap<Uuid, Vec<Uuid>>> {
        let mut nodes_indices = HashMap::<Uuid, Vec<Uuid>>::new();
        for node_idx in self.g.node_indices() {
            let node_ref = self.node_by_idx(node_idx)?;
            if node_ref.uuid()? == node_id {
                nodes_indices.entry(group_id).or_default().push(node_id);
            }
            let node = node_ref.optical_ref.lock_opm()?;
            let node_attrs = node.node_attr().clone();
            if let Some(group) = node.as_any().downcast_ref::<NodeGroup>() {
                let ref_nodes_map = group
                    .graph()
                    .find_all_nodes_referring_to_uuid(node_id, group.node_attr.uuid())?;
                for (gid, ref_nodes) in ref_nodes_map {
                    nodes_indices.entry(gid).or_default().extend(ref_nodes);
                }
            }
            drop(node);
            if node_attrs.node_type() == "reference" {
                let ref_node_props = node_attrs.properties();
                if let Ok(Proptype::Uuid(ref_uuid)) = ref_node_props.get("reference id")
                    && *ref_uuid == node_id
                {
                    nodes_indices
                        .entry(group_id)
                        .or_default()
                        .push(node_attrs.uuid());
                }
            }
        }
        Ok(nodes_indices)
    }

    /// Delete all edges connected to a node identified by `node_index`.
    pub fn delete_edges_of_node(&mut self, node_index: NodeIndex) {
        self.delete_edges_of_node_with_direction(node_index, Direction::Incoming);
        self.delete_edges_of_node_with_direction(node_index, Direction::Outgoing);
    }

    /// Delete all edges connected to `node_index` in the specified [`Direction`].
    pub fn delete_edges_of_node_with_direction(&mut self, node_index: NodeIndex, dir: Direction) {
        let edge_indices: Vec<EdgeIndex> = self
            .g
            .edges_directed(node_index, dir)
            .map(|e| e.id())
            .collect();

        for edge_idx in edge_indices {
            self.g.remove_edge(edge_idx);
        }
    }

    /// Connect two optical nodes within this [`OpticGraph`].
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - the [`NodeIndex`] of source or target node does not exist.
    ///   - a port name of the source or target node does not exist.
    ///   - a node/port combination was already connected.
    ///   - connecting the nodes would form a loop in the network.
    ///   - the given distance is not finite.
    pub fn connect_nodes(
        &mut self,
        src_id: Uuid,
        src_port: &str,
        target_id: Uuid,
        target_port: &str,
        distance: Length,
    ) -> OpmResult<()> {
        if self.is_inverted() {
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
            .lock_opm()?
            .ports()
            .names(&PortType::Output)
            .contains(&src_port.into())
        {
            let src_ports = source
                .optical_ref
                .lock_opm()?
                .ports()
                .names(&PortType::Output)
                .join(", ");
            return Err(OpossumError::OpticScenery(format!(
                "source node {} does not have an output port {src_port}. Possible values are: {src_ports}",
                source.optical_ref.lock_opm()?
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
            .lock_opm()?
            .ports()
            .names(&PortType::Input)
            .contains(&target_port.into())
        {
            let target_ports = target
                .optical_ref
                .lock_opm()?
                .ports()
                .names(&PortType::Input)
                .join(", ");
            return Err(OpossumError::OpticScenery(format!(
                "target node {} does not have an input port {target_port}. Possible values are: {target_ports}",
                target.optical_ref.lock_opm()?
            )));
        }
        if self.src_node_port_exists(src_node, src_port) {
            return Err(OpossumError::OpticScenery(format!(
                "src node <{}> with port <{src_port}> is already connected",
                source.optical_ref.lock_opm()?
            )));
        }
        if self.target_node_port_exists(target_node, target_port) {
            return Err(OpossumError::OpticScenery(format!(
                "target node {} with port <{target_port}> is already connected",
                target.optical_ref.lock_opm()?
            )));
        }
        let src_name = source.optical_ref.lock_opm()?.name().to_string();
        let target_name = target.optical_ref.lock_opm()?.name().to_string();
        let light = LightFlow::new(src_port, target_port, distance)?;
        let edge_index = self.g.add_edge(src_node, target_node, light);
        if is_cyclic_directed(&self.g) {
            self.g.remove_edge(edge_index);
            return Err(OpossumError::OpticScenery(format!(
                "connecting nodes <{src_name}> -> <{target_name}> would form a loop"
            )));
        }
        // Remove port mappings if they are no longer valid
        self.input_port_map.remove(target_id, target_port);
        self.output_port_map.remove(src_id, src_port);
        Ok(())
    }

    /// Disconnect two optical nodes within this [`OpticGraph`].
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
            Err(OpossumError::OpticScenery(format!(
                "source node {} with port <{src_port}> is not connected",
                node_ref.optical_ref.lock_opm()?
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
            if let Some(edge) = self.g.edge_weight_mut(edge_id) {
                edge.set_distance(distance)?;
            }
            Ok(())
        } else {
            let node_ref = self.node(src_id)?;
            Err(OpossumError::OpticScenery(format!(
                "source node {} with port <{src_port}> is not connected",
                node_ref.optical_ref.lock_opm()?
            )))
        }
    }

    /// Update the connections of a single inverted node.
    ///
    /// # Errors
    ///
    /// This function will return an error if the node with the given [`Uuid`] does not exist.
    pub fn update_connections_of_single_inverted_node(&mut self, node_id: Uuid) -> OpmResult<()> {
        let node_index = self.node_idx_by_uuid(node_id).ok_or_else(|| {
            OpossumError::OpticScenery("node with given index does not exist".into())
        })?;

        let outgoing_edges: Vec<(EdgeIndex, NodeIndex, NodeIndex, LightFlow)> = self
            .g
            .edges_directed(node_index, Direction::Outgoing)
            .map(|e| (e.id(), e.target(), e.source(), e.weight().clone()))
            .collect();
        let incoming_edges: Vec<(EdgeIndex, NodeIndex, NodeIndex, LightFlow)> = self
            .g
            .edges_directed(node_index, Direction::Incoming)
            .map(|e| (e.id(), e.target(), e.source(), e.weight().clone()))
            .collect();

        self.delete_edges_of_node(node_index);

        // Fixed check to verify both input AND output port maps
        if !self.input_port_map.contains_node(node_id)
            && !self.output_port_map.contains_node(node_id)
        {
            if let Some(changed_node) = self.g.node_weight(node_index).cloned() {
                let optical_ref = changed_node.optical_ref.lock_opm()?;
                let ports = optical_ref.ports();
                let input_ports = ports.ports(&PortType::Input).clone();
                let output_ports = ports.ports(&PortType::Output).clone();
                drop(optical_ref);
                if output_ports.len() == 1 && input_ports.len() == 1 {
                    if let Some(outgoing_edge) = outgoing_edges.first()
                        && outgoing_edges.len() == 1
                        && let (Some((output_port, _)), Some(target_node)) = (
                            output_ports.first_key_value(),
                            self.g.node_weight(outgoing_edge.1),
                        )
                    {
                        self.connect_nodes(
                            node_id,
                            output_port,
                            target_node.uuid()?,
                            outgoing_edge.3.target_port(),
                            *outgoing_edge.3.distance(),
                        )?;
                    }

                    if let Some(incoming_edge) = incoming_edges.first()
                        && incoming_edges.len() == 1
                        && let (Some((input_port, _)), Some(src_node)) = (
                            input_ports.first_key_value(),
                            self.g.node_weight(incoming_edge.2),
                        )
                    {
                        self.connect_nodes(
                            src_node.uuid()?,
                            incoming_edge.3.src_port(),
                            node_id,
                            input_port,
                            *incoming_edge.3.distance(),
                        )?;
                    }
                }
            }
        } else {
            // Remove port mappings if involved in external group mapping
            self.input_port_map.remove_all_from_uuid(node_id);
            self.output_port_map.remove_all_from_uuid(node_id);
        }

        Ok(())
    }

    /// Inverts the [`OpticGraph`].
    ///
    /// This function changes all directions of node connections and inverts the nodes themselves.
    /// # Errors
    ///
    /// This function will return an error if trying to invert a graph containing a non-invertible node.
    pub fn invert_graph(&mut self) -> OpmResult<()> {
        for node in self.g.node_weights_mut() {
            let node_to_be_inverted = !node.optical_ref.lock_opm()?.inverted();

            node.optical_ref
                .lock_opm()?
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

    fn external_nodes(&self, port_type: PortType) -> Vec<NodeIndex> {
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
                .lock_opm()
                .unwrap()
                .ports()
                .names(&port_type)
                .len();
            if ports != edges {
                nodes.push(node_idx);
            }
        }
        nodes
    }

    /// Remove a port mapping. Returns `true` if successful.
    pub fn remove_mapped_port(&mut self, external_name: &str, port_type: PortType) -> bool {
        match port_type {
            PortType::Input => self.input_port_map.remove_key(external_name),
            PortType::Output => self.output_port_map.remove_key(external_name),
        }
    }

    /// Map a port of an internal node to an external port of the group.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - An external port name has already been assigned.
    /// - The `node_id` or `internal_name` does not exist.
    /// - The node is not an external node of the group.
    /// - The port is already internally connected.
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
        if !self.external_nodes(*port_type).contains(&node_idx) {
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
            .lock_opm()?
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

    fn edges_directed(&self, idx: NodeIndex, dir: Direction) -> Edges<'_, LightFlow, Directed> {
        self.g.edges_directed(idx, dir)
    }
}
