use std::collections::HashSet;

use actix_web::web::{self};
use opossum_core::{
    error::OpmResult,
    meter,
    nodes::{ConnectionInfo, NodeGroup},
    opm_document::OpmDocument,
    types::api_types::ConnectInfo,
};
use uuid::Uuid;

use super::graph_lookup::is_reference_target;
use crate::{app_state::AppState, error::BackEndErrorResponse};

/// Builds a [`ConnectInfo`] from raw connection endpoints, enriching it with whether the target
/// node is a reference node (see [`is_reference_target`]) - the flag the GUI uses to style
/// reference edges differently.
pub fn build_connect_info(
    scenery: &NodeGroup,
    src_id: Uuid,
    src_port: &str,
    target_id: Uuid,
    target_port: &str,
    distance: f64,
) -> ConnectInfo {
    let is_reference = is_reference_target(scenery, target_id);
    ConnectInfo::new(
        src_id,
        src_port.to_string(),
        target_id,
        target_port.to_string(),
        distance,
        is_reference,
    )
}

/// Split and classify connections relative to a set of nodes.
///
/// Connections are categorized into three groups:
/// - `inside`: connections where both source and target nodes are inside the set
/// - `input`: connections entering the set (target inside, source outside)
/// - `output`: connections leaving the set (source inside, target outside)
///
/// Additionally, each connection is annotated with whether its target node
/// represents a reference node.
///
/// # Arguments
///
/// * `connections` - Slice of all connections to evaluate.
/// * `nodes` - Slice of node UUIDs defining the subset of interest.
///
/// # Returns
///
/// Returns a [`ConnectionSplit`] struct containing the categorized connections.
///
/// # Errors
///
/// Missing node attributes are treated as non-reference nodes.
#[allow(clippy::significant_drop_tightening)]
pub fn split_sort_connections(
    data: &web::Data<AppState>,
    connections: &[ConnectionInfo],
    nodes: &[Uuid],
) -> ConnectionSplit {
    let document = data.document.lock();
    split_sort_connections_from_document(&document, connections, nodes)
}

/// Same classification as [`split_sort_connections`], but taking an already-locked `&OpmDocument`
/// directly instead of `&web::Data<AppState>` - for callers (like undo/redo command application) that
/// only have a document reference, not the full app state, and shouldn't re-lock it.
pub(crate) fn split_sort_connections_from_document(
    document: &OpmDocument,
    connections: &[ConnectionInfo],
    nodes: &[Uuid],
) -> ConnectionSplit {
    let node_set: HashSet<Uuid> = nodes.iter().copied().collect();

    let mut split = ConnectionSplit {
        inside: Vec::new(),
        input: Vec::new(),
        output: Vec::new(),
    };

    let scenery = document.scenery();
    for c in connections {
        let is_reference = is_reference_target(scenery, c.target_id);
        let c_info = ConnectInfo::from_connection_info(c, is_reference);

        let src_inside = node_set.contains(&c_info.src_uuid());
        let tgt_inside = node_set.contains(&c_info.target_uuid());

        match (src_inside, tgt_inside) {
            (true, true) => split.inside.push(c_info),
            (true, false) => split.output.push(c_info),
            (false, true) => split.input.push(c_info),
            _ => {}
        }
    }

    split
}

/// Represents a categorized split of connections.
///
/// # Fields
///
/// * `inside` - Connections fully contained within the node set.
/// * `input` - Connections entering the node set.
/// * `output` - Connections leaving the node set.
pub struct ConnectionSplit {
    pub inside: Vec<ConnectInfo>,
    pub input: Vec<ConnectInfo>,
    pub output: Vec<ConnectInfo>,
}

/// Connect two nodes within a group based on `ConnectInfo`.
///
/// This is a convenience helper that forwards connection data to
/// `NodeGroup::connect_nodes`.
///
/// # Arguments
///
/// * `group` - The group in which the connection should be created.
/// * `conn` - The connection description.
///
/// # Errors
///
/// This function will return an error if the connection cannot be created.
pub(crate) fn connect_from_info(group: &mut NodeGroup, conn: &ConnectInfo) -> OpmResult<()> {
    group.connect_nodes(
        conn.src_uuid(),
        conn.src_port(),
        conn.target_uuid(),
        conn.target_port(),
        meter!(conn.distance()),
    )
}

/// Reconnects every [`ConnectInfo`] in `connections` inside `group_id`'s graph, via
/// [`connect_from_info`].
///
/// # Errors
///
/// Returns an error if `group_id` doesn't resolve to a group, or a connection can't be re-created
/// (e.g. a referenced node/port no longer exists).
pub fn reconnect_all(
    document: &mut OpmDocument,
    group_id: Uuid,
    connections: &[ConnectInfo],
) -> Result<(), BackEndErrorResponse> {
    for conn in connections {
        document
            .scenery_mut()
            .with_group_node_mut(group_id, |g| connect_from_info(g, conn))??;
    }
    Ok(())
}
