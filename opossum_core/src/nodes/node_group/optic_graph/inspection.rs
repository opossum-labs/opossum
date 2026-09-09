use super::{ConnectionInfo, OpticGraph};
use crate::{
    core_optics::{OpticRef, PortType},
    error::{OpmResult, OpossumError},
    nodes::NodeGroup,
};
use petgraph::{Direction, algo::connected_components, graph::NodeIndex, visit::EdgeRef};
use uuid::Uuid;

impl OpticGraph {
    /// Return `true` if the node with the given [`Uuid`] is not connected to any other node.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given `node_id` is not found
    pub fn is_stale_node(&self, node_id: Uuid) -> OpmResult<bool> {
        let idx = self
            .node_idx_by_uuid(node_id)
            .ok_or_else(|| OpossumError::Analysis("uuid does not exist".into()))?;
        let neighbors = self.g.neighbors_undirected(idx);
        Ok(neighbors.count() == 0 && !self.input_port_map.contains_node(node_id))
    }
    /// Return `true` if the node with the given [`Uuid`] has no incoming connections.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given `node_id` does not exist in the graph.
    pub fn has_no_incoming_connections(&self, node_id: Uuid) -> OpmResult<bool> {
        let idx = self
            .node_idx_by_uuid(node_id)
            .ok_or_else(|| OpossumError::Analysis("uuid does not exist".into()))?;
        let neighbors = self.g.neighbors_directed(idx, Direction::Incoming);
        Ok(neighbors.count() == 0 && !self.input_port_map.contains_node(node_id))
    }
    /// Return `true` if the node with the given [`Uuid`] has at least one incoming connection.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given `node_id` does not exist in the graph.
    pub fn has_input_connections(&self, node_id: Uuid) -> OpmResult<bool> {
        self.has_no_incoming_connections(node_id).map(|res| !res)
    }
    /// Return `true` if the node with the given [`Uuid`] has at least one outgoing connection.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given `node_id` is not found.
    pub fn has_output_connections(&self, node_id: Uuid) -> OpmResult<bool> {
        let idx = self
            .node_idx_by_uuid(node_id)
            .ok_or_else(|| OpossumError::Analysis("uuid does not exist".into()))?;
        let neighbors = self.g.neighbors_directed(idx, Direction::Outgoing);
        Ok(neighbors.count() != 0 || self.output_port_map.contains_node(node_id))
    }
    /// Returns `true` if this [`OpticGraph`] consists of a single connected tree.
    ///
    /// The graph is considered single-tree if all nodes are connected (ignoring edge directions).
    #[must_use]
    pub fn is_single_tree(&self) -> bool {
        if self.is_empty() {
            return true;
        }
        connected_components(&self.g) == 1
    }
    /// Returns the (optical) node with the given [`Uuid`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the node with the given [`Uuid`] does not exist
    /// or if retrieving the node's UUID fails.
    pub fn node(&self, uuid: Uuid) -> OpmResult<OpticRef> {
        // 1. Map items to a Result tuple containing the evaluated UUID matching state
        let found_node = self
            .g
            .node_weights()
            .map(|node| {
                // Evaluate node.uuid() which now returns an OpmResult<Uuid>
                node.uuid().map(|node_uuid| (node_uuid == uuid, node))
            })
            // 2. Find the first item that either matches or encountered an error
            .find(|res| match res {
                Ok((is_match, _)) => *is_match,
                Err(_) => true, // Keep the error so we can propagate it via '?'
            });

        // 3. Process the find result and convert it to the final OpmResult<OpticRef>
        match found_node {
            Some(Ok((_, node))) => Ok(node.clone()),
            Some(Err(err)) => Err(err), // Propagate the internal error (e.g., poisoning)
            None => Err(OpossumError::OpticScenery(
                "node with given uuid does not exist".into(),
            )),
        }
    }
    /// Returns a mutable reference to the (optical) node with the given [`Uuid`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the node with the given [`Uuid`] does not exist.
    pub fn node_mut(&mut self, uuid: Uuid) -> OpmResult<&mut OpticRef> {
        let idx = self.node_idx_by_uuid(uuid).ok_or_else(|| {
            OpossumError::OpticScenery("node with given uuid does not exist".into())
        })?;
        Ok(&mut self.g[idx])
    }
    /// Returns a reference to the optical node specified by its [`Uuid`] and the Uuid of the group in which it is contained.
    ///
    /// This function is similar to [`OpticGraph::node`] but also checks recursively for
    /// the node in all sub-groups.
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn node_recursive(&self, uuid: Uuid, group_id: Uuid) -> OpmResult<(OpticRef, Uuid)> {
        if let Ok(node) = self.node(uuid) {
            Ok((node, group_id))
        } else {
            for node_ref in self.g.node_weights() {
                if let Some(group) = node_ref.as_any().downcast_ref::<NodeGroup>()
                    && let Ok((node, group_id)) = group.node_recursive(uuid)
                {
                    return Ok((node, group_id));
                }
            }
            Err(OpossumError::OpticScenery(
                "node with given uuid does not exist".into(),
            ))
        }
    }
    /// Recursively search for a mutable reference to the node with the given `uuid` in this graph
    /// and all nested sub-groups.
    ///
    /// # Errors
    /// Returns [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node_recursive_mut(&mut self, uuid: Uuid) -> OpmResult<&mut OpticRef> {
        if let Some(idx) = self.node_idx_by_uuid(uuid) {
            return Ok(&mut self.g[idx]);
        }
        for node in self.g.node_weights_mut() {
            if let Some(group) = node.as_any_mut().downcast_mut::<NodeGroup>()
                && let Ok(target) = group.graph_mut().node_recursive_mut(uuid)
            {
                return Ok(target);
            }
        }
        Err(OpossumError::OpticScenery(
            "node with given uuid does not exist".into(),
        ))
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
    /// `None` is returned if the node with the given [`Uuid`] does not exist
    /// or if an error occurs while accessing a node's UUID.
    #[must_use]
    pub fn node_idx_by_uuid(&self, uuid: Uuid) -> Option<NodeIndex> {
        self.g.node_indices().find(|&idx| {
            // Safely get the node weight without unwrap
            self.g
                .node_weight(idx)
                .is_some_and(|node| match node.uuid() {
                    Ok(node_uuid) => node_uuid == uuid,
                    Err(err) => {
                        // Log the error to maintain visibility in the homelab/server environment
                        log::error!("Failed to retrieve UUID for node at index {idx:?}: {err:?}");
                        false
                    }
                })
        })
    }
    /// Returns all nodes of this [`OpticGraph`].
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
            let src_id = self
                .g
                .node_weight(edge_ref.source())
                .unwrap()
                .uuid()
                .unwrap();
            let target = self.g.node_weight(edge_ref.target()).unwrap();
            let target_id = target.uuid().unwrap();
            let src_port = edge_ref.weight().src_port();
            let target_port = edge_ref.weight().target_port();
            let dist = edge_ref.weight().distance();
            let connection = ConnectionInfo {
                src_id,
                src_port: src_port.to_string(),
                target_id,
                target_port: target_port.to_string(),
                distance: *dist,
            };
            connections.push(connection);
        }
        connections
    }

    /// Returns all outgoing connections from the node with the given [`Uuid`].
    ///
    /// Each outgoing connection is represented as a [`ConnectionInfo`], including
    /// source and target IDs, port names, and distance.
    ///
    /// # Parameters
    /// - `node_id`: UUID of the source node whose outgoing connections are requested.
    ///
    /// # Returns
    /// A vector of [`ConnectionInfo`] representing all outgoing edges of the node.
    ///
    /// # Panics
    /// This function may panic if the internal graph contains edges referencing non-existent nodes.
    #[must_use]
    pub fn get_outgoing_connection_info_of_node(&self, node_id: Uuid) -> Vec<ConnectionInfo> {
        let mut connections = Vec::<ConnectionInfo>::new();
        for edge_ref in self.g.edge_references() {
            let src_id = self
                .g
                .node_weight(edge_ref.source())
                .unwrap()
                .uuid()
                .unwrap();
            if node_id == src_id {
                let connection = ConnectionInfo {
                    src_id,
                    src_port: edge_ref.weight().src_port().to_string(),
                    target_id: self
                        .g
                        .node_weight(edge_ref.target())
                        .unwrap()
                        .uuid()
                        .unwrap(),
                    target_port: edge_ref.weight().target_port().to_string(),
                    distance: *edge_ref.weight().distance(),
                };
                connections.push(connection);
            }
        }
        connections
    }

    /// Returns all connections from the node with the given [`Uuid`].
    ///
    /// Each connection is represented as a [`ConnectionInfo`], including
    /// source and target IDs, port names, and distance.
    ///
    /// # Parameters
    /// - `node_id`: UUID of the source node whose connections are requested.
    ///
    /// # Returns
    /// A vector of [`ConnectionInfo`] representing all edges of the node.
    ///
    /// # Panics
    /// This function may panic if the internal graph contains edges referencing non-existent nodes.
    #[must_use]
    pub fn get_connection_info_of_node(&self, node_id: Uuid) -> Vec<ConnectionInfo> {
        let mut connections = Vec::<ConnectionInfo>::new();
        for edge_ref in self.g.edge_references() {
            let src = self.g.node_weight(edge_ref.source()).unwrap();
            let target = self.g.node_weight(edge_ref.target()).unwrap();
            if node_id == src.uuid().unwrap() || node_id == target.uuid().unwrap() {
                let connection = ConnectionInfo {
                    src_id: src.uuid().unwrap(),
                    src_port: edge_ref.weight().src_port().to_string(),
                    target_id: target.uuid().unwrap(),
                    target_port: edge_ref.weight().target_port().to_string(),
                    distance: *edge_ref.weight().distance(),
                };
                connections.push(connection);
            }
        }
        connections
    }

    /// Returns all incoming connections from the node with the given [`Uuid`].
    ///
    /// Each incoming connection is represented as a [`ConnectionInfo`], including
    /// source and target IDs, port names, and distance.
    ///
    /// # Parameters
    /// - `node_id`: UUID of the source node whose incoming connections are requested.
    ///
    /// # Returns
    /// A vector of [`ConnectionInfo`] representing all incoming edges of the node.
    ///
    /// # Panics
    /// This function may panic if the internal graph contains edges referencing non-existent nodes.
    #[must_use]
    pub fn get_incoming_connection_info_of_node(&self, node_id: Uuid) -> Vec<ConnectionInfo> {
        let mut connections = Vec::<ConnectionInfo>::new();
        for edge_ref in self.g.edge_references() {
            let target_id = self
                .g
                .node_weight(edge_ref.target())
                .unwrap()
                .uuid()
                .unwrap();
            if node_id == target_id {
                let connection = ConnectionInfo {
                    src_id: self
                        .g
                        .node_weight(edge_ref.source())
                        .unwrap()
                        .uuid()
                        .unwrap(),
                    src_port: edge_ref.weight().src_port().to_string(),
                    target_id,
                    target_port: edge_ref.weight().target_port().to_string(),
                    distance: *edge_ref.weight().distance(),
                };
                connections.push(connection);
            }
        }
        connections
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
    /// Returns `true` if the node is an input node.
    ///
    /// This function checks if a node with the given [`NodeIndex`] has an unconnected input port.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given `node_ide` is not found.
    pub fn is_incoming_node(&self, node_id: Uuid) -> OpmResult<bool> {
        let nr_of_input_ports = self.node(node_id)?.ports().ports(&PortType::Input).len();
        let idx = self
            .node_idx_by_uuid(node_id)
            .ok_or_else(|| OpossumError::OpticGroup("node_id does not exist".into()))?;
        let nr_of_incoming_edges = self.g.edges_directed(idx, Direction::Incoming).count();
        Ok(nr_of_incoming_edges < nr_of_input_ports)
    }
    /// Returns `true` if the node is an output node.
    ///
    /// This function checks if a node with the given [`NodeIndex`] has an unconnected output port.
    ///
    /// # Errors
    ///
    /// This functions returns na error if
    /// - the given `node_id` does not exist.
    pub fn is_output_node(&self, node_id: Uuid) -> OpmResult<bool> {
        let ports = self.node(node_id)?.ports();
        let nr_of_output_ports = ports.ports(&PortType::Output).len();
        let idx = self
            .node_idx_by_uuid(node_id)
            .ok_or_else(|| OpossumError::OpticGroup("node_id does not exist".into()))?;
        let nr_of_outgoing_edges = self.g.edges_directed(idx, Direction::Outgoing).count();
        debug_assert!(
            nr_of_outgoing_edges <= nr_of_output_ports,
            "# of outgoing edges > # of output ports ???"
        );
        Ok(nr_of_outgoing_edges < nr_of_output_ports)
    }
    /// Recursively finds all nodes of type "source port" and returns their UUIDs.
    ///
    /// # Errors
    ///
    /// This function returns an error if node access fails.
    pub fn find_source_ports(&self) -> OpmResult<Vec<Uuid>> {
        let mut source_ports = Vec::new();
        for node_ref in self.nodes() {
            let node_id = node_ref.uuid()?;
            if node_ref.node_attr().node_type() == "source port" {
                source_ports.push(node_id);
            }
            if let Some(group) = node_ref.as_any().downcast_ref::<NodeGroup>() {
                let sub_ports = group.graph.find_source_ports()?;
                source_ports.extend(sub_ports);
            }
        }
        Ok(source_ports)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::nodes::{Dummy, NodeGroup};
    use num_traits::Zero;
    use uom::si::f64::Length;
    #[test]
    fn node_by_uuid() -> OpmResult<()> {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default())?;
        assert!(graph.node(n1).is_ok());
        assert!(graph.node(Uuid::nil()).is_err());
        Ok(())
    }
    #[test]
    fn node_id() -> OpmResult<()> {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default())?;
        assert!(graph.node_idx_by_uuid(n1).is_some());
        assert!(graph.node_idx_by_uuid(Uuid::nil()).is_none());
        Ok(())
    }
    #[test]
    fn is_single_tree() -> OpmResult<()> {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default())?;
        let n2 = graph.add_node(Dummy::default())?;
        let n3 = graph.add_node(Dummy::default())?;
        let n4 = graph.add_node(Dummy::default())?;
        graph.connect_nodes(n1, "output_1", n2, "input_1", Length::zero())?;
        graph.connect_nodes(n3, "output_1", n4, "input_1", Length::zero())?;
        assert_eq!(graph.is_single_tree(), false);
        graph.connect_nodes(n2, "output_1", n3, "input_1", Length::zero())?;
        assert_eq!(graph.is_single_tree(), true);
        Ok(())
    }

    #[test]
    fn has_input_connections_node_not_found() {
        let graph = OpticGraph::default();
        let result = graph.has_input_connections(Uuid::nil());
        assert!(result.is_err());
    }

    #[test]
    fn has_input_connections_no_edges() -> OpmResult<()> {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default())?;

        let result = graph.has_input_connections(n1)?;
        assert!(!result);
        Ok(())
    }

    #[test]
    fn has_input_connections_with_edge() -> OpmResult<()> {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default())?;
        let n2 = graph.add_node(Dummy::default())?;

        graph.connect_nodes(n1, "output_1", n2, "input_1", Length::zero())?;

        assert!(!graph.has_input_connections(n1)?);
        assert!(graph.has_input_connections(n2)?);
        Ok(())
    }

    #[test]
    fn has_input_connections_mapped_input_port() -> OpmResult<()> {
        // This reproduces the NodeGroup port mapping case
        let mut group = NodeGroup::default();

        let n1 = group.add_node(Dummy::default())?;

        // No edges, but expose the port via the group
        group.map_input_port(n1, "input_1", "group_input")?;

        // Internally this means the graph must report an input connection
        assert!(group.graph.has_input_connections(n1)?);
        Ok(())
    }

    #[test]
    fn has_output_connections_none() -> OpmResult<()> {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default())?;

        assert!(!graph.has_output_connections(n1)?);
        Ok(())
    }

    #[test]
    fn has_output_connections_with_edge() -> OpmResult<()> {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default())?;
        let n2 = graph.add_node(Dummy::default())?;

        graph.connect_nodes(n1, "output_1", n2, "input_1", Length::zero())?;

        assert!(graph.has_output_connections(n1)?);
        Ok(())
    }

    #[test]
    fn has_output_connections_only_incoming() -> OpmResult<()> {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default())?;
        let n2 = graph.add_node(Dummy::default())?;

        graph.connect_nodes(n1, "output_1", n2, "input_1", Length::zero())?;

        assert!(!graph.has_output_connections(n2)?);
        Ok(())
    }

    #[test]
    fn has_output_connections_uuid_error() {
        let graph = OpticGraph::default();
        assert!(graph.has_output_connections(Uuid::nil()).is_err());
    }

    #[test]
    fn has_output_connections_when_output_mapped() -> OpmResult<()> {
        let mut group = NodeGroup::default();

        let n1 = group.add_node(Dummy::default())?;

        group.map_output_port(n1, "output_1", "output_1")?;

        assert!(group.graph.has_output_connections(n1)?);
        Ok(())
    }

    #[test]
    fn has_output_connections_already_assigned() -> OpmResult<()> {
        let mut group = NodeGroup::default();

        let n1 = group.add_node(Dummy::default())?;
        let n2 = group.add_node(Dummy::default())?;

        group.connect_nodes(n1, "output_1", n2, "input_1", Length::zero())?;

        assert!(group.map_output_port(n1, "output_1", "output_1").is_err());
        Ok(())
    }

    #[test]
    fn has_output_connections_no_edge_no_mapping() -> OpmResult<()> {
        let mut group = NodeGroup::default();

        let n1 = group.add_node(Dummy::default())?;

        assert!(!group.graph.has_output_connections(n1)?);
        Ok(())
    }
}
