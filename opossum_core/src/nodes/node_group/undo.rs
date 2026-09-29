#![warn(missing_docs)]
//! # Undo Dispatcher for Node Groups
//!
//! Provides reversal mechanisms for all mutations described by [`GraphDelta`]
//! across hierarchical [`NodeGroup`] setups.

use super::NodeGroup;
use crate::{
    core_optics::node_attr::HasNodeAttr,
    error::OpmResult,
    nodes::node_group::optic_graph::{
        OpticGraph,
        delta::{GraphDeletionDelta, GraphDelta},
    },
};
use uuid::Uuid;

impl NodeGroup {
    /// Executes a mutating closure on the [`OpticGraph`] of either this group (if `target_group_id` matches)
    /// or targets the corresponding nested subgroup recursively.
    fn with_target_group_graph_mut<R>(
        &mut self,
        target_group_id: Uuid,
        f: impl FnOnce(&mut OpticGraph) -> OpmResult<R>,
    ) -> OpmResult<R> {
        if self.node_attr().uuid() == target_group_id {
            f(&mut self.graph)
        } else {
            self.with_group_node_mut(target_group_id, |group| f(group.graph_mut()))?
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
        match delta {
            GraphDelta::NodeAdded { group_id, node_id } => {
                self.with_target_group_graph_mut(*group_id, |g| {
                    g.remove_node_no_cascade(*node_id)
                })?;
            }
            GraphDelta::NodeDeleted(deletion_delta) => {
                self.revert_deletion(deletion_delta)?;
            }
            GraphDelta::NodesConnected {
                group_id,
                connection,
                displaced_port_mappings,
            } => {
                self.with_target_group_graph_mut(*group_id, |g| {
                    g.disconnect_nodes(connection.src_id, &connection.src_port)
                })?;
                for mapping in displaced_port_mappings {
                    self.with_target_group_graph_mut(mapping.group_id, |g| {
                        g.map_port(
                            mapping.internal_node_id,
                            &mapping.port_type,
                            &mapping.internal_port_name,
                            &mapping.external_name,
                        )
                    })?;
                }
            }
            GraphDelta::NodesDisconnected {
                group_id,
                connection,
            } => {
                self.with_target_group_graph_mut(*group_id, |g| {
                    g.connect_nodes(
                        connection.src_id,
                        &connection.src_port,
                        connection.target_id,
                        &connection.target_port,
                        connection.distance,
                    )
                })?;
            }
            GraphDelta::ConnectionDistanceChanged {
                group_id,
                src_id,
                src_port,
                old_distance,
                ..
            } => {
                self.with_target_group_graph_mut(*group_id, |g| {
                    g.update_connection_distance(*src_id, src_port, *old_distance)
                })?;
            }
            GraphDelta::PortMapped(mapping) => {
                self.with_target_group_graph_mut(mapping.group_id, |g| {
                    g.remove_mapped_port(&mapping.external_name, mapping.port_type);
                    Ok(())
                })?;
            }
            GraphDelta::PortUnmapped(mapping) => {
                self.with_target_group_graph_mut(mapping.group_id, |g| {
                    g.map_port(
                        mapping.internal_node_id,
                        &mapping.port_type,
                        &mapping.internal_port_name,
                        &mapping.external_name,
                    )
                })?;
            }
            GraphDelta::Composite(deltas) => {
                for sub_delta in deltas.iter().rev() {
                    self.apply_undo(sub_delta)?;
                }
            }
        }
        Ok(())
    }

    /// Reverts a multi-entity deletion by re-inserting nodes, port mappings, and edges in phased order.
    fn revert_deletion(&mut self, delta: &GraphDeletionDelta) -> OpmResult<()> {
        // Phase 1: Re-insert all nodes into their original parent groups
        for record in &delta.deleted_nodes {
            self.with_target_group_graph_mut(record.parent_group_id, |g| {
                g.add_node_ref(record.node.clone()).map(|_| ())
            })?;
        }

        // Phase 2: Re-resolve references across all hierarchy levels
        self.graph.resolve_all_references()?;

        // Phase 3: Restore exposed port mappings
        for mapping in &delta.removed_port_mappings {
            self.with_target_group_graph_mut(mapping.group_id, |g| {
                g.map_port(
                    mapping.internal_node_id,
                    &mapping.port_type,
                    &mapping.internal_port_name,
                    &mapping.external_name,
                )
            })?;
        }

        // Phase 4: Reconnect edges
        for conn_rec in &delta.removed_connections {
            let conn = &conn_rec.connection;
            self.with_target_group_graph_mut(conn_rec.parent_group_id, |g| {
                g.connect_nodes(
                    conn.src_id,
                    &conn.src_port,
                    conn.target_id,
                    &conn.target_port,
                    conn.distance,
                )
            })?;
        }

        Ok(())
    }
}
