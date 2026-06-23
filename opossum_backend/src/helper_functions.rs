use std::{collections::HashSet, pin::Pin};

use actix_web::{
    FromRequest, HttpRequest, dev::Payload, web::{self},
};
use nalgebra::Point2;
use opossum_core::{
    core_optics::OpticRef,
    error::OpmResult,
    meter,
    nodes::{ConnectionInfo, NodeGroup},
    types::api_types::{ConnectInfo, NodeInfo},
    utils::LockExt,
};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::{app_state::AppState, error::BackEndErrorResponse};

/// Collect the optical references and top-left position of the given nodes.
///
/// Iterates over all provided node UUIDs, resolves their corresponding
/// `OpticRef`s, and determines the minimum `(x, y)` GUI position among them.
/// The returned position can be used as an anchor point for placing a new group.
///
/// # Arguments
///
/// * `nodes_to_convert` - Slice of node UUIDs to collect.
///
/// # Returns
///
/// Returns a tuple containing:
/// - `Vec<OpticRef>`: The resolved optical references of the nodes.
/// - `Point2<f64>`: The minimum `(x, y)` position among all nodes.
///
/// # Notes
///
/// Nodes that cannot be resolved are silently ignored.
#[allow(clippy::significant_drop_tightening)]
pub fn collect_node_refs_and_pos(
    data: &web::Data<AppState>,
    nodes_to_convert: &[Uuid],
) -> (Vec<OpticRef>, Point2<f64>) {
    let document = data.document.lock();
    let scenery = document.scenery();
    let mut corner = Point2::new(f64::INFINITY, f64::INFINITY);
    let optic_ref_vec = nodes_to_convert
        .iter()
        .filter_map(|node| {
            scenery.node_recursive(*node).ok().map(|(r, _)| {
                if let Ok(opt_ref) = r.optical_ref.lock_opm() {
                    let pos = opt_ref.gui_position().unwrap();
                    corner.x = corner.x.min(pos.x);
                    corner.y = corner.y.min(pos.y);
                }
                r
            })
        })
        .collect();
    (optic_ref_vec, corner)
}

/// Collect all connections of the given group.
///
/// # Arguments
///
/// * `group_id` - The UUID of the group whose connections should be retrieved.
///
/// # Returns
///
/// Returns a vector of `ConnectionInfo` representing all connections within the group.
///
/// # Errors
///
/// This function will return an error if the `group_id` was not found.
#[allow(clippy::significant_drop_tightening)]
pub fn collect_group_connections(
    data: &web::Data<AppState>,
    group_id: Uuid,
) -> OpmResult<Vec<ConnectionInfo>> {
    let document = data.document.lock();
    let scenery = document.scenery();

    scenery.with_group_node(group_id, opossum_core::nodes::NodeGroup::connections)
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
    let node_set: HashSet<Uuid> = nodes.iter().copied().collect();

    let mut split = ConnectionSplit {
        inside: Vec::new(),
        input: Vec::new(),
        output: Vec::new(),
    };

    let document = data.document.lock();
    let scenery = document.scenery();
    for c in connections {
        let is_reference = scenery
            .with_node_attr(c.target_id, |attr| {
                attr.properties().get("reference id").is_ok()
            })
            .unwrap_or(false);

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

/// Build a new group node from the given node references and classified connections.
///
/// The new group will:
/// - Contain all provided node references
/// - Preserve internal connections between nodes (`connections.inside`)
/// - Map input ports based on incoming connections (`connections.input`)
/// - Map output ports based on outgoing connections (`connections.output`)
///
/// # Arguments
///
/// * `node_refs` - Optical references of nodes to include in the group.
/// * `connections` - A [`ConnectionSplit`] containing categorized connections:
///     - `inside`: connections fully within the group
///     - `input`: connections entering the group
///     - `output`: connections leaving the group
///
/// # Returns
///
/// Returns the constructed `NodeGroup`.
///
/// # Errors
///
/// This function will return an error if:
/// - Adding a node reference fails
/// - Creating internal connections fails
/// - Mapping input or output ports fails
pub fn build_new_group_from_refs_and_conns(
    node_refs: Vec<OpticRef>,
    connections: &ConnectionSplit,
) -> OpmResult<NodeGroup> {
    let mut new_group = NodeGroup::new("new group");

    for node_ref in node_refs {
        new_group.add_node_ref(node_ref)?;
    }

    for conn in &connections.inside {
        connect_from_info(&mut new_group, conn)?;
    }

    for map_out in &connections.output {
        new_group.map_output_port(map_out.src_uuid(), map_out.src_port(), map_out.src_port())?;
    }

    for map_in in &connections.input {
        new_group.map_input_port(
            map_in.target_uuid(),
            map_in.target_port(),
            map_in.target_port(),
        )?;
    }

    Ok(new_group)
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
pub fn connect_from_info(group: &mut NodeGroup, conn: &ConnectInfo) -> OpmResult<()> {
    group.connect_nodes(
        conn.src_uuid(),
        conn.src_port(),
        conn.target_uuid(),
        conn.target_port(),
        meter!(conn.distance()),
    )
}

/// Replace a set of nodes with a newly created group node in the scenery.
///
/// The function:
/// - Removes all specified nodes from the source group
/// - Inserts the new group node
/// - Reconnects external input and output connections
///
/// # Arguments
///
/// * `group_id` - The UUID of the group containing the original nodes.
/// * `nodes_to_convert` - List of node UUIDs to remove and replace.
/// * `new_group` - The constructed group node to insert.
/// * `map_input_connections` - Connections entering the new group.
/// * `map_output_connections` - Connections leaving the new group.
///
/// # Returns
///
/// Returns the UUID of the newly inserted group node.
///
/// # Errors
///
/// This function will return an error if:
/// - The group was not found
/// - Any node deletion fails
/// - The new group cannot be inserted
/// - Reconnecting external connections fails
#[allow(clippy::significant_drop_tightening)]
pub fn add_converted_group_to_scenery(
    data: &web::Data<AppState>,
    group_id: Uuid,
    mut nodes_to_convert: Vec<Uuid>,
    new_group: NodeGroup,
    map_input_connections: &[ConnectInfo],
    map_output_connections: &[ConnectInfo],
) -> Result<Uuid, BackEndErrorResponse> {
    let mut document = data.document.lock();
    let scenery = document.scenery_mut();
    while let Some(node) = nodes_to_convert.pop() {
        let deleted = scenery.delete_node(node)?;
        for del_id in &deleted {
            nodes_to_convert.retain(|id| id != del_id);
        }
    }

    scenery.with_group_node_mut(group_id, |g| {
        match g.add_node(new_group) {
            Ok(new_group_id) => {
                //connect the output ports and connect within scenery
                for map_out in map_output_connections {
                    connect_from_info(g, map_out)?;
                }
                //connect the input ports
                for map_in in map_input_connections {
                    connect_from_info(g, map_in)?;
                }
                Ok(new_group_id)
            }
            Err(e) => Err(BackEndErrorResponse::new(
                404,
                "Opossum",
                &format!("Could not add group node{e}"),
            )),
        }
    })?
}

/// Create a [`NodeInfo`] representation for a newly created group node.
///
/// # Arguments
///
/// * `new_group_id` - The UUID of the new group node.
/// * `pos` - The position where the node should be placed.
///
/// # Returns
///
/// Returns a `NodeInfo` describing the group node, including its ports and position.
///
/// # Errors
///
/// This function will return an error if the node cannot be resolved
/// or if its data cannot be accessed.
#[allow(clippy::significant_drop_tightening)]
pub fn create_new_group_node_info(
    data: &web::Data<AppState>,
    new_group_id: Uuid,
    pos: Point2<f64>,
) -> OpmResult<NodeInfo> {
    let document = data.document.lock();
    let scenery = document.scenery();

    let (new_group_ref, _) = scenery.node_recursive(new_group_id)?;
    let new_group_node = new_group_ref.optical_ref.lock_opm()?;

    Ok(NodeInfo::from_analyzable(
        &*new_group_node,
        Some(Some((pos.x, pos.y))),
    ))
}

/// Custom extractor to handle Rusty Object Notation (RON) payloads
pub struct Ron<T>(pub T);

impl<T> Ron<T> {
    /// Deconstruct to get the inner value
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> FromRequest for Ron<T>
where
    T: DeserializeOwned + 'static,
{
    // Use your custom error response type directly
    type Error = BackEndErrorResponse;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        // Reuse Actix's built-in String extractor to read the request body
        let string_fut = String::from_request(req, payload);

        Box::pin(async move {
            // 1. Extract the raw string payload and map potential Actix errors
            let body_str = string_fut.await.map_err(|err| {
                BackEndErrorResponse::new(
                    400,
                    "Payload Error",
                    &format!("Failed to read request body: {err}"),
                )
            })?;
            
            // 2. Deserialize the RON string into the target type T
            let data = ron::de::from_str(&body_str).map_err(|err| {
                BackEndErrorResponse::new(
                    400,
                    "Parse Error",
                    &format!("Failed to deserialize payload: {err}"),
                )
            })?;

            Ok(Ron(data))
        })
    }
}