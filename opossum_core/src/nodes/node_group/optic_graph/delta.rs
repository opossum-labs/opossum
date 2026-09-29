use crate::{
    core_optics::{OpticRef, PortType},
    nodes::node_group::ConnectionInfo,
};
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;
use uuid::Uuid;

/// Details of an external port mapping removed or restored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovedPortMapping {
    /// ID of the group where the port mapping was registered.
    pub group_id: Uuid,
    /// Type of the port (Input or Output).
    pub port_type: PortType,
    /// Exposed external port name.
    pub external_name: String,
    /// Internal node target ID.
    pub internal_node_id: Uuid,
    /// Internal port name on the target node.
    pub internal_port_name: String,
}

/// Snapshot of a deleted node paired with its owning group ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletedNodeRecord {
    /// ID of the group that structurally contained this node.
    pub parent_group_id: Uuid,
    /// Full snapshot of the optical node at the time of deletion.
    pub node: OpticRef,
}

/// A removed connection (edge) paired with the ID of its containing group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedConnectionRecord {
    /// ID of the group where this connection existed.
    pub parent_group_id: Uuid,
    /// Full connection metadata.
    pub connection: ConnectionInfo,
}

/// Comprehensive delta describing all entities removed during a node deletion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphDeletionDelta {
    /// Target node UUID requested for deletion.
    pub target_node_id: Uuid,
    /// All nodes removed (including nested nodes and cascading references).
    pub deleted_nodes: Vec<DeletedNodeRecord>,
    /// All connections removed across the graph hierarchy.
    pub removed_connections: Vec<RemovedConnectionRecord>,
    /// All port mappings removed during cascading cleanup.
    pub removed_port_mappings: Vec<RemovedPortMapping>,
}

impl GraphDeletionDelta {
    /// Merge another deletion delta into this one.
    pub fn merge(&mut self, other: GraphDeletionDelta) {
        self.deleted_nodes.extend(other.deleted_nodes);
        self.removed_connections.extend(other.removed_connections);
        self.removed_port_mappings
            .extend(other.removed_port_mappings);
    }
}

/// Unified delta representing any state mutation in the optical graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphDelta {
    /// A single node was added.
    NodeAdded {
        /// Parent group containing the new node.
        group_id: Uuid,
        /// UUID of the newly added node.
        node_id: Uuid,
    },
    /// A node was deleted (along with cascading edges, references, and mappings).
    NodeDeleted(GraphDeletionDelta),
    /// Two nodes were connected by an edge.
    NodesConnected {
        /// Group owning the connection.
        group_id: Uuid,
        /// Information about the established connection.
        connection: ConnectionInfo,
        /// Port mappings displaced by this connection.
        displaced_port_mappings: Vec<RemovedPortMapping>,
    },
    /// A connection between two nodes was disconnected.
    NodesDisconnected {
        /// Group owning the connection.
        group_id: Uuid,
        /// Information about the severed connection.
        connection: ConnectionInfo,
    },
    /// Optical propagation distance on an existing edge was modified.
    ConnectionDistanceChanged {
        /// Group owning the connection.
        group_id: Uuid,
        /// Source node UUID.
        src_id: Uuid,
        /// Source port name.
        src_port: String,
        /// Distance before mutation.
        old_distance: Length,
        /// Distance after mutation.
        new_distance: Length,
    },
    /// An internal node port was mapped to a group port.
    PortMapped(RemovedPortMapping),
    /// A group port mapping was removed.
    PortUnmapped(RemovedPortMapping),
    /// An atomic composite of multiple graph deltas.
    Composite(Vec<GraphDelta>),
}
