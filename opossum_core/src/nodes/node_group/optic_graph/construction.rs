use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use super::graph::OpticGraph;
use crate::{
    analyzers::Analyzable,
    core_optics::OpticRef,
    error::{OpmResult, OpossumError},
    light::LightFlow,
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
    /// **Note**: While constructing the underlying `OpticRef` a random, `UUID` is assigned.
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
        // Paranoia check: Ensure UUID does not already exist
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
    /// This function is similar to [`OpticGraph::add_node`] but allows to add an existing `OpticRef` to the graph.
    ///
    /// # Errors
    ///
    /// This function will return an error if the graph is set as `inverted` and a node is added. (This could end up in
    /// a weird / undefined behaviour)
    pub fn add_node_ref(&mut self, node: OpticRef) -> OpmResult<NodeIndex> {
        if self.is_inverted() {
            return Err(OpossumError::OpticGroup(
                "cannot add nodes if group is set as inverted".into(),
            ));
        }
        Ok(self.g.add_node(node))
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
    ///
    /// # Panics
    /// This function could theoretically panic if the uuid of the node is not found while looping over all nodes.
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
            // If we have already processed this UUID, skip it.
            if !processed_uuids.insert(current_id_to_check) {
                continue;
            }
            // This inner loop finds all nodes that are or reference the current_id_to_check
            while let Some(node_idx) = self.find_first_node_with_uuid(current_id_to_check) {
                // We have to get the uuid of the node, which could be the (initially) given uuid or the uuid of a reference node
                let actual_node_id = self.node_by_idx(node_idx).unwrap().uuid();
                // collect all node ids of nodes that are contained in a group
                if let Ok(node_ref) = self.node_by_idx(node_idx) {
                    let node = node_ref.optical_ref.lock_opm()?;
                    if let Ok(group) = node.as_group()
                        && let Ok(sub_ids) = group.collect_all_contained_node_ids_recursive()
                    {
                        for id in sub_ids {
                            deletion_queue.push(id);
                            nodes_deleted.push(id);
                        }
                    }
                }
                self.g.remove_node(node_idx);
                // Remove possibly no longer valid port mappings
                self.input_port_map.remove_all_from_uuid(actual_node_id);
                self.output_port_map.remove_all_from_uuid(actual_node_id);

                if !nodes_deleted.contains(&actual_node_id) {
                    nodes_deleted.push(actual_node_id);
                }
                // Add the UUID of the node we just deleted to the queue.
                // This ensures we will now search for any nodes that referenced *it*.
                deletion_queue.push(actual_node_id);
            }
        }
        // now check if subnodes exist and delete recusively
        for node_ref in self.nodes() {
            let mut node = node_ref.optical_ref.lock_opm()?;
            if let Ok(group) = node.as_group_mut()
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
        // Remove duplicates that might occur from the subgroup recursion
        nodes_deleted.sort();
        nodes_deleted.dedup();
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
    fn find_first_node_with_uuid(&self, node_id: Uuid) -> Option<NodeIndex> {
        for node_idx in self.g.node_indices() {
            let node_ref = self.node_by_idx(node_idx).unwrap();
            if node_ref.uuid() == node_id {
                return Some(node_idx);
            }
            let node = node_ref.optical_ref.lock_opm().unwrap();
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

    /// Return a vector of [`Uuid`] of nodes that reference a certain node with the given [`Uuid`] in this [`OpticGraph`].
    ///
    /// This includes:
    /// - The node itself if it matches `node_id`
    /// - Any reference nodes that refer to the given `node_id`
    /// - Recursively any nodes inside groups that reference the `node_id`
    ///
    /// Returns an empty vector if no node with (or referring to) the given `node_id` was found.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The UUID of the node to find references to.
    ///
    /// # Returns
    ///
    /// Returns a `Vec<Uuid>` containing the UUIDs of all nodes referring to the specified `node_id`.
    ///
    /// # Errors
    ///
    /// Returns an [`OpmResult::Err`] if:
    /// - Accessing a node by index fails
    /// - Locking a node’s `OpticRef` fails
    /// - Any recursive group traversal fails
    pub fn find_all_nodes_referring_to_uuid(
        &self,
        node_id: Uuid,
        group_id: Uuid,
    ) -> OpmResult<HashMap<Uuid, Vec<Uuid>>> {
        let mut nodes_indices = HashMap::<Uuid, Vec<Uuid>>::new();
        for node_idx in self.g.node_indices() {
            let node_ref = self.node_by_idx(node_idx)?;
            if node_ref.uuid() == node_id {
                if let Some(node_refs) = nodes_indices.get_mut(&group_id) {
                    node_refs.push(node_id);
                } else {
                    nodes_indices.insert(group_id, vec![node_id]);
                }
            }
            let node = node_ref.optical_ref.lock_opm()?;
            let node_attrs = node.node_attr().clone();
            if let Ok(group) = node.as_group() {
                let ref_nodes_map = group
                    .graph()
                    .find_all_nodes_referring_to_uuid(node_id, group.node_attr.uuid())?;
                for (group_id, ref_nodes) in &ref_nodes_map {
                    if let Some(node_refs) = nodes_indices.get_mut(group_id) {
                        node_refs.extend(ref_nodes);
                    } else {
                        nodes_indices.insert(*group_id, ref_nodes.clone());
                    }
                }
                // nodes_indices.extend(group.graph().find_all_nodes_referring_to_uuid(node_id, group.node_attr.uuid())?);
            }
            drop(node);
            if node_attrs.node_type() == "reference" {
                let ref_node_props = node_attrs.properties();
                if let Ok(Proptype::Uuid(ref_uuid)) = ref_node_props.get("reference id")
                    && *ref_uuid == node_id
                {
                    if let Some(node_refs) = nodes_indices.get_mut(&group_id) {
                        node_refs.push(node_attrs.uuid());
                    } else {
                        nodes_indices.insert(group_id, vec![node_attrs.uuid()]);
                    }
                    // nodes_indices.push(node_attrs.uuid());
                }
            }
        }
        Ok(nodes_indices)
    }
    /// Delete all edges of a node with the [`NodeIndex`] `node_index`
    pub fn delete_edges_of_node(&mut self, node_index: NodeIndex) {
        self.delete_edges_of_node_with_direction(node_index, Direction::Incoming);
        self.delete_edges_of_node_with_direction(node_index, Direction::Outgoing);
    }
    /// Delete all edges of a node with the [`NodeIndex`] `node_index` and the [`Direction`] `dir`.
    ///
    /// A simple loop might not work, as the call `remove_edge` re-indexes the remaining edges
    pub fn delete_edges_of_node_with_direction(&mut self, node_index: NodeIndex, dir: Direction) {
        while self.g.edges_directed(node_index, dir).count() != 0 {
            let edge_idx_vec = self
                .g
                .edges_directed(node_index, dir)
                .map(|e| e.id())
                .collect::<Vec<EdgeIndex>>();

            if let Some(idx) = edge_idx_vec.first() {
                self.g.remove_edge(*idx);
            }
        }
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
                "source node {} does not have an output port {}. Possible values are: {}",
                source.optical_ref.lock_opm()?,
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
                "target node {} does not have an input port {}. Possible values are: {}",
                target.optical_ref.lock_opm()?,
                target_port,
                target_ports
            )));
        }
        if self.src_node_port_exists(src_node, src_port) {
            return Err(OpossumError::OpticScenery(format!(
                "src node <{}> with port <{}> is already connected",
                source.optical_ref.lock_opm()?,
                src_port
            )));
        }
        if self.target_node_port_exists(target_node, target_port) {
            return Err(OpossumError::OpticScenery(format!(
                "target node {} with port <{}> is already connected",
                target.optical_ref.lock_opm()?,
                target_port
            )));
        }
        let src_name = source.optical_ref.lock_opm()?.name();
        let target_name = target.optical_ref.lock_opm()?.name();
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
            let edge = self.g.edge_weight_mut(edge_id);
            if let Some(edge) = edge {
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
    /// This function is used to update the connections of a single inverted node. It removes all
    /// connections of the node and connects it to the next node in the graph.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The [`Uuid`] of the node to update.
    ///
    /// # Errors
    ///
    /// This function will return an error if the node with the given [`Uuid`] does not exist.
    ///
    /// # Panics
    ///
    /// This function will panic if the mutex lock fails.
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

        if !self.input_port_map.contains_node(node_id)
            && !self.input_port_map.contains_node(node_id)
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
                            target_node.uuid(),
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
                            src_node.uuid(),
                            incoming_edge.3.src_port(),
                            node_id,
                            input_port,
                            *incoming_edge.3.distance(),
                        )?;
                    }
                }
            }
        } else {
            //todo, what about port mapping when inverting a group or a node inside a group that is mapped to an input or output of the group?
            //as for now, if this node is involved in any kind of port mapping delete all edges an remove its mapping
            self.input_port_map.remove_all_from_uuid(node_id);
            self.output_port_map.remove_all_from_uuid(node_id);
        }

        Ok(())
    }
    /// Inverts the [`OpticGraph`].
    ///
    /// This functions changes all directions of node connections and inverts the nodes itself.
    /// # Errors
    ///
    /// This function will return an error if one tries to invert a graph containing a non-invertable node (eg. source).
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

    /// Remove a port mapping
    ///
    /// Returns true if successful
    pub fn remove_mapped_port(&mut self, external_name: &str, port_type: PortType) -> bool {
        match port_type {
            PortType::Input => self.input_port_map.remove_key(external_name),
            PortType::Output => self.output_port_map.remove_key(external_name),
        }
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
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        core_optics::OpticNode,
        millimeter,
        nodes::{BeamSplitter, Dummy, NodeGroup, NodeReference, SplittingConfigBuilder},
    };
    use num::Zero;
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
    fn add_node_duplicate_uuid() {
        let mut og = OpticGraph::default();
        let node1 = Dummy::default();
        // We clone the node to get a second instance with the exact same UUID.
        let node2 = node1.clone();
        let id1 = og.add_node(node1).unwrap();
        let result = og.add_node(node2);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert_eq!(
            err_msg,
            format!("OpticGroup:node with uuid {id1} already exists")
        );
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
        let sub_node3 =
            BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.5)).unwrap();
        let sn3_i = og.add_node(sub_node3).unwrap();
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
            .unwrap();
        og.connect_nodes(sn2_i, "output_1", sn3_i, "input_1", Length::zero())
            .unwrap();
        assert_eq!(og.external_nodes(PortType::Input), vec![0.into(), 2.into()])
    }
    #[test]
    fn output_nodes() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sub_node1 =
            BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.5)).unwrap();
        let sn2_i = og.add_node(sub_node1).unwrap();
        let sn3_i = og.add_node(Dummy::default()).unwrap();
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
            .unwrap();
        og.connect_nodes(sn2_i, "out1_trans1_refl2", sn3_i, "input_1", Length::zero())
            .unwrap();
        assert_eq!(og.external_nodes(PortType::Input), vec![0.into(), 1.into()])
    }
    #[test]
    fn next_node_with_uuid_single() {
        let mut graph = OpticGraph::default();
        let i_d1 = graph.add_node(Dummy::default()).unwrap();
        let i_d2 = graph.add_node(Dummy::default()).unwrap();
        let ref_node = NodeReference::from_node(&graph.node(i_d1).unwrap());
        let _ = graph.add_node(ref_node).unwrap();

        assert!(graph.find_first_node_with_uuid(Uuid::nil()).is_none());
        let mut nodes = vec![];
        while let Some(node_idx) = graph.find_first_node_with_uuid(i_d2) {
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
        while let Some(node_idx) = graph.find_first_node_with_uuid(i_d1) {
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
    fn delete_node_with_nested_refs() {
        let mut graph = OpticGraph::default();
        let i_d = graph.add_node(Dummy::default()).unwrap();
        let ref_node = NodeReference::from_node(&graph.node(i_d).unwrap());
        let i_ref = graph.add_node(ref_node).unwrap();
        let ref_node = NodeReference::from_node(&graph.node(i_ref).unwrap());
        let i_ref_ref = graph.add_node(ref_node).unwrap();
        assert_eq!(graph.g.node_count(), 3);
        let deleted_nodes = graph.delete_node(i_d).unwrap();
        assert!(deleted_nodes.contains(&i_d));
        assert!(deleted_nodes.contains(&i_ref));
        assert!(deleted_nodes.contains(&i_ref_ref));
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
    fn delete_group_node_collects_subnodes() {
        let mut graph = OpticGraph::default();

        let _i_d1 = graph.add_node(Dummy::default()).unwrap();

        let mut group = NodeGroup::default();
        let i_g_d1 = group.add_node(Dummy::default()).unwrap();
        let i_g_d2 = group.add_node(Dummy::default()).unwrap();
        let i_group = graph.add_node(group).unwrap();

        assert_eq!(graph.g.node_count(), 2);

        let deleted_nodes = graph.delete_node(i_group).unwrap();

        assert_eq!(graph.g.node_count(), 1);
        assert!(deleted_nodes.contains(&i_group));
        assert!(deleted_nodes.contains(&i_g_d1));
        assert!(deleted_nodes.contains(&i_g_d2));
        assert!(deleted_nodes.len() == 3);
    }
    #[test]
    fn delete_group_node_with_reference_node() {
        let mut graph = OpticGraph::default();

        let i_root = graph.add_node(Dummy::default()).unwrap();

        let mut group = NodeGroup::default();
        let ref_node = NodeReference::from_node(&graph.node(i_root).unwrap());
        let i_ref = group.add_node(ref_node).unwrap();
        let i_group = graph.add_node(group).unwrap();

        let deleted_nodes = graph.delete_node(i_group).unwrap();

        assert!(deleted_nodes.contains(&i_group));
        assert!(deleted_nodes.contains(&i_ref));
        assert!(!deleted_nodes.contains(&i_root));
    }
    #[test]
    fn delete_nested_group_nodes() {
        let mut graph = OpticGraph::default();

        let i_root = graph.add_node(Dummy::default()).unwrap();

        let mut inner_group = NodeGroup::default();
        let ref_node = NodeReference::from_node(&graph.node(i_root).unwrap());
        let i_ref = inner_group.add_node(ref_node).unwrap();
        let i_inner_group = NodeGroup::default();
        let i_inner_group_node = inner_group.add_node(i_inner_group).unwrap();

        let mut outer_group = NodeGroup::default();
        let i_outer_group = outer_group.add_node(inner_group).unwrap();
        let i_outer_group_node = graph.add_node(outer_group).unwrap();

        let deleted_nodes = graph.delete_node(i_outer_group_node).unwrap();

        assert!(deleted_nodes.contains(&i_outer_group_node));
        assert!(deleted_nodes.contains(&i_outer_group));
        assert!(deleted_nodes.contains(&i_ref));
        assert!(deleted_nodes.contains(&i_inner_group_node));
    }
    #[test]
    fn delete_group_node_reference_oustide() {
        let mut graph = OpticGraph::default();

        let dummy = Dummy::default();
        let mut group = NodeGroup::default();
        let i_dummy = group.add_node(dummy).unwrap();
        let dummy_ref = NodeReference::from_node(&group.graph.node(i_dummy).unwrap());
        let i_dummy_ref = graph.add_node(dummy_ref).unwrap();
        let i_group = graph.add_node(group).unwrap();

        let deleted_nodes = graph.delete_node(i_group).unwrap();

        assert!(deleted_nodes.contains(&i_group));
        assert!(deleted_nodes.contains(&i_dummy));
        assert!(deleted_nodes.contains(&i_dummy_ref));
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

        assert_eq!(
            graph
                .node_recursive(i_d1, uuid::Uuid::nil())
                .unwrap()
                .0
                .uuid(),
            i_d1
        );
        assert_eq!(
            graph
                .node_recursive(i_d2, uuid::Uuid::nil())
                .unwrap()
                .0
                .uuid(),
            i_d2
        );
        assert!(
            graph
                .node_recursive(uuid::Uuid::nil(), uuid::Uuid::nil())
                .is_err()
        );
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

        let group_id = group.node_attr().uuid();
        let i_g = graph.add_node(group).unwrap();
        assert_eq!(graph.node_recursive(i_d, group_id).unwrap().0.uuid(), i_d);
        assert_eq!(graph.node_recursive(i_g, group_id).unwrap().0.uuid(), i_g);
        assert_eq!(
            graph.node_recursive(i_g_d1, group_id).unwrap().0.uuid(),
            i_g_d1
        );
        assert_eq!(
            graph.node_recursive(i_g_d2, group_id).unwrap().0.uuid(),
            i_g_d2
        );
        assert_eq!(
            graph.node_recursive(i_g_g2, group_id).unwrap().0.uuid(),
            i_g_g2
        );
        assert_eq!(
            graph.node_recursive(i_g_g2_d, group_id).unwrap().0.uuid(),
            i_g_g2_d
        );
    }
}
