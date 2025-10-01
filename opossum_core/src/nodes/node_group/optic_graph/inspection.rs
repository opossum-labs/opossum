use super::{ConnectionInfo, OpticGraph};
use crate::{
    error::{OpmResult, OpossumError},
    optic_ports::PortType,
    optic_ref::OpticRef,
};
use petgraph::{Direction, algo::connected_components, graph::NodeIndex, visit::EdgeRef};
use uuid::Uuid;

impl OpticGraph {
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
    /// Returns a node with the given [`Uuid`].
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
                let mut node = node_ref
                    .optical_ref
                    .lock()
                    .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?;

                if let Ok(group) = node.as_group_mut()
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
            let src_id = self.g.node_weight(edge_ref.source()).unwrap().uuid();
            let target_id = self.g.node_weight(edge_ref.target()).unwrap().uuid();
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
    /// Returns `true` if the node is an input node.
    ///
    /// This function checks if a node with the given [`NodeIndex`] has an unconnected input port.
    ///
    /// # Panics
    ///
    /// Panics if an error occurs while locking the mutex.
    #[must_use]
    pub fn is_incoming_node(&self, node_id: Uuid) -> bool {
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
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::nodes::Dummy;
    use num::Zero;
    use uom::si::f64::Length;
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
}
