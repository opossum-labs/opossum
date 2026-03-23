use std::collections::{HashMap, HashSet};

use dioxus::html::geometry::euclid::default::Point2D;
use opossum_core::nodes::NodeAttr;
use opossum_core::nodes::fluence_detector::Fluence;
use opossum_core::opm_document::AnalyzerInfo;
use opossum_core::prelude::*;
use opossum_core::types::api_types::{ConnectInfo, NewNode, NewRefNode, NodeInfo};
use uuid::Uuid;

use crate::HTTP_API_CLIENT;

/// Get all nodes in the current scenery
///
/// # Errors
///
/// This function will return an error if
/// - the request fails (e.g. the scenery is not valid)
/// - the response cannot be deserialized into a vector of [`NodeInfo`] structs
pub async fn get_nodes(group_id: Uuid) -> Result<Vec<NodeInfo>, String> {
    HTTP_API_CLIENT()
        .get::<Vec<NodeInfo>>(&format!("/api/scenery/{}/nodes", group_id.as_simple()))
        .await
}
/// Get a list of all connections (edges) of the given node group.
///
/// # Errors
///
/// This function will return an error if
/// - the given `node_id` does not correspond to a (sub-)group of the scenery or the scenery itself.
pub async fn get_connections(group_id: Uuid) -> Result<Vec<ConnectInfo>, String> {
    HTTP_API_CLIENT()
        .get::<Vec<ConnectInfo>>(&format!(
            "/api/scenery/{}/connections",
            group_id.as_simple()
        ))
        .await
}
/// Send a request to add a node to the scenery.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`NewNode`] cannot be serialized
/// - the request fails (e.g. the node type is not valid)
/// - the `group_id` does not exist
/// - the response cannot be deserialized into the [`NodeInfo`] struct
pub async fn post_add_node(new_node_info: NewNode, group_id: Uuid) -> Result<NodeInfo, String> {
    HTTP_API_CLIENT()
        .post::<NewNode, NodeInfo>(
            &format!("/api/scenery/{}/nodes", group_id.as_simple()),
            new_node_info,
        )
        .await
}

/// Send a request to copy nodes of the scenery.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`Uuid`] cannot be serialized
/// - the request fails (e.g. the node ide does not exist)
pub async fn post_copy_nodes(nodes: HashSet<Uuid>) -> Result<String, String> {
    HTTP_API_CLIENT()
        .post::<HashSet<Uuid>, String>("/api/scenery/nodes_copy", nodes)
        .await
}

pub async fn post_paste_nodes(
    group_id: Uuid,
    pos: Point2D<f64>,
) -> Result<(Vec<NodeInfo>, Vec<AnalyzerInfo>, Vec<ConnectInfo>), String> {
    HTTP_API_CLIENT()
        .post::<(Uuid, (f64, f64)), (Vec<NodeInfo>, Vec<AnalyzerInfo>, Vec<ConnectInfo>)>(
            "/api/scenery/nodes_paste",
            (group_id, (pos.x, pos.y)),
        )
        .await
}

/// Send a request to add a reference node to the scenery.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`NewRefNode`] cannot be serialized
/// - the provided [`Uuid`] of the node to be referred to does not exist
/// - the `group_id` does not exist
/// - the response cannot be deserialized into the [`NodeInfo`] struct
pub async fn post_add_ref_node(
    new_ref_info: NewRefNode,
    group_id: Uuid,
) -> Result<NodeInfo, String> {
    HTTP_API_CLIENT()
        .post::<NewRefNode, NodeInfo>(
            &format!("/api/scenery/{}/references", group_id.as_simple()),
            new_ref_info,
        )
        .await
}
/// Delete a node and all its connections.
///
/// This function will return a vector of [`Uuid`]s that were actually deleted. This could include
/// the provided [`Uuid`] and possibly any other nodes that reference it.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`Uuid`] cannot be serialized or found
/// - the returned response cannot be deserialized into a vector of [`Uuid`]
pub async fn delete_node(id: Uuid) -> Result<Vec<Uuid>, String> {
    HTTP_API_CLIENT()
        .delete::<String, Vec<Uuid>>(
            &format!("/api/scenery/{}/nodes", id.as_simple()),
            String::new(),
        )
        .await
}
/// Get the properties of an optical node.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`Uuid`] cannot be serialized or found
/// - the properties cannot be deserialized into the [`NodeAttr`] struct
pub async fn get_node_properties(uuid: Uuid) -> Result<(NodeAttr, bool), String> {
    HTTP_API_CLIENT()
        .get_ron::<(NodeAttr, bool)>(&format!("/api/scenery/{}/properties", uuid.as_simple()))
        .await
}

/// Get the information about an analyzer node.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`Uuid`] cannot be serialized or found
/// - the properties cannot be deserialized into the [`AnalyzerInfo`] struct
pub async fn get_analyzer_info(uuid: Uuid) -> Result<AnalyzerInfo, String> {
    HTTP_API_CLIENT()
        .get_ron::<AnalyzerInfo>(&format!("/api/scenery/{}/analyzer_info", uuid.as_simple()))
        .await
}

/// Connect two nodes.
///
/// # Errors
///
/// This function will return an error if the provided [`ConnectInfo`] cannot be serialized or if the request fails.
pub async fn post_add_connection(
    connection: ConnectInfo,
    group_id: Uuid,
) -> Result<ConnectInfo, String> {
    HTTP_API_CLIENT()
        .post::<ConnectInfo, ConnectInfo>(
            &format!("/api/scenery/{}/connection", group_id.as_simple()),
            connection,
        )
        .await
}
/// Delete a connection between two nodes.
///
/// # Errors
///
/// This function will return an error if the provided [`ConnectInfo`] cannot be serialized or if the request fails.
pub async fn delete_connection(
    connection: ConnectInfo,
    group_id: Uuid,
) -> Result<ConnectInfo, String> {
    HTTP_API_CLIENT()
        .delete::<ConnectInfo, ConnectInfo>(
            &format!("/api/scenery/{}/connection", group_id.as_simple()),
            connection,
        )
        .await
}
/// Update the physical distance between two nodes.
///
/// # Errors
///
/// This function will return an error if the connection could not be found.
pub async fn update_distance(
    connection: ConnectInfo,
    group_id: Uuid,
) -> Result<ConnectInfo, String> {
    HTTP_API_CLIENT()
        .put::<ConnectInfo, ConnectInfo>(
            &format!("/api/scenery/{}/connection", group_id.as_simple()),
            connection,
        )
        .await
}
/// Update the GUI position coordinates of the node with the given `node_id`.
///
/// # Errors
///
/// This function will return an error if the `node_id` was not found.
pub async fn update_gui_position(
    node_id: Uuid,
    gui_position: Point2D<f64>,
) -> Result<String, String> {
    let position = (gui_position.x, gui_position.y);
    HTTP_API_CLIENT()
        .post::<(f64, f64), String>(
            &format!("/api/scenery/position/{}", node_id.as_simple()),
            position,
        )
        .await
}

/// Update the name of the node with the given `node_id`.
///
/// # Errors
///
/// This function will return an error if the `node_id` was not found.
pub async fn update_node_name(
    node_id: Uuid,
    node_name: String,
) -> Result<HashMap<Uuid, String>, String> {
    HTTP_API_CLIENT()
        .post::<String, HashMap<Uuid, String>>(
            &format!("/api/scenery/name/{}", node_id.as_simple()),
            node_name,
        )
        .await
}

/// Update the lidt of the node with the given `node_id`.
///
/// # Errors
///
/// This function will return an error if the `node_id` was not found.
pub async fn update_node_lidt(node_id: Uuid, node_lidt: Fluence) -> Result<String, String> {
    HTTP_API_CLIENT()
        .post::<Fluence, String>(
            &format!("/api/scenery/lidt/{}", node_id.as_simple()),
            node_lidt,
        )
        .await
}

/// Update the alignment of the node with the given `node_id`.
///
/// # Errors
/// This function will return an error if the `node_id` was not found or if the alignment cannot be serialized.
pub async fn update_node_alignment(node_id: Uuid, alignment: Isometry) -> Result<String, String> {
    HTTP_API_CLIENT()
        .post::<Isometry, String>(
            &format!("/api/scenery/alignmentisometry/{}", node_id.as_simple()),
            alignment,
        )
        .await
}

pub async fn get_group_hierarchy(group_id: Uuid) -> Result<Vec<(Uuid, String)>, String> {
    HTTP_API_CLIENT()
        .get::<Vec<(Uuid, String)>>(&format!("/api/scenery/{}/hierarchy", group_id.as_simple()))
        .await
}

pub async fn convert_nodes_to_group(
    nodes: Vec<Uuid>,
    group_id: Uuid,
) -> Result<(NodeInfo, Vec<ConnectInfo>), String> {
    HTTP_API_CLIENT()
        .post::<Vec<Uuid>, (NodeInfo, Vec<ConnectInfo>)>(
            &format!("/api/groups/{}/convert_to_group", group_id.as_simple()),
            nodes,
        )
        .await
}

/// Update the property of the node with the given `node_id`.
/// The property value is already passes as a `serde_json::Value` to avoid implementing `PartialEq` for every property type.
///
/// # Errors
///
/// This function will return an error if the `node_id` was not found.
pub async fn update_node_property(
    node_id: Uuid,
    property_key_val: (String, Proptype),
) -> Result<String, String> {
    HTTP_API_CLIENT()
        .post_ron::<(String, Proptype), String>(
            &format!("/api/scenery/property/{}", node_id.as_simple()),
            property_key_val,
        )
        .await
}

/// Updates the isometry (position and orientation) of a node in the scenery.
///
/// This function sends a POST request to the server to update the [`Isometry`] associated
/// with a specific node identified by its UUID. The server endpoint is:
/// `/api/scenery/isometry/{node_id}`.
///
/// # Parameters
/// - `client`: An instance of [`HTTPClient`] used to send the request.
/// - `node_id`: The unique identifier of the node whose isometry is to be updated.
/// - `iso`: The new [`Isometry`] data to apply to the node.
///
/// # Returns
/// A [`Result`] containing:
/// - `Ok(String)`: The server's response if the update is successful.
/// - `Err(String)`: An error message returned from the server or the HTTP client.
///
/// # Errors
/// This function returns an error if:
/// - The HTTP request fails to reach the server (e.g., network issues).
/// - The server responds with an error status code (e.g., 4xx or 5xx).
/// - Serialization of the [`Isometry`] payload fails before sending.
pub async fn update_node_isometry(node_id: Uuid, iso: Option<Isometry>) -> Result<String, String> {
    HTTP_API_CLIENT()
        .post::<Option<Isometry>, String>(
            &format!("/api/scenery/isometry/{}", node_id.as_simple()),
            iso,
        )
        .await
}

/// Update the inversion state of a node.
/// This function sends a POST request to the server to update whether the node is inverted or not.
/// The server endpoint is:
/// `/api/scenery/inversion/{node_id}`.
///
/// # Parameters
/// - `client`: An instance of [`HTTPClient`] used to send the request.
/// - `node_id`: The unique identifier of the node whose inversion state is to be updated.
/// - `inverted`: A boolean indicating whether the node should be inverted or not.
///
/// # Returns
/// A [`Result`] containing:
/// - `Ok(String)`: The server's response if the update is successful.
/// - `Err(String)`: An error message returned from the server or the HTTP client.
///
/// # Errors
/// This function returns an error if:
/// - The HTTP request fails to reach the server (e.g., network issues).
/// - The server responds with an error status code (e.g., 4xx or 5xx).
/// - Serialization of the boolean payload fails before sending.
pub async fn update_node_inversion(
    node_id: Uuid,
    inverted: bool,
) -> Result<Vec<ConnectInfo>, String> {
    HTTP_API_CLIENT()
        .post::<bool, Vec<ConnectInfo>>(
            &format!("/api/scenery/inversion/{}", node_id.as_simple()),
            inverted,
        )
        .await
}

/// Update the analyzer configuration of an analyzer node.
/// This function sends a POST request to the server to update the analyzer type
/// associated with a specific node identified by its UUID. The server endpoint is:
/// `/api/scenery/analyzer/{node_id}`.
///
/// # Parameters
/// - `client`: An instance of [`HTTPClient`] used to send the request.
/// - `node_id`: The unique identifier of the node whose analyzer configuration is to be updated.
/// - `analyzer_type`: The new [`AnalyzerType`] to apply to the node.
/// # Returns
/// A [`Result`] containing:
/// - `Ok(String)`: The server's response if the update is successful.
/// - `Err(String)`: An error message returned from the server or the HTTP client.
/// # Errors
/// This function returns an error if:
/// - The HTTP request fails to reach the server (e.g., network issues).
/// - The server responds with an error status code (e.g., 4xx or 5xx).
/// - Serialization of the [`AnalyzerType`] payload fails before sending.
pub async fn update_analyzer_config_ron(
    node_id: Uuid,
    analyzer_type: AnalyzerType,
) -> Result<String, String> {
    HTTP_API_CLIENT()
        .post_ron::<AnalyzerType, String>(
            &format!("/api/scenery/analyzer/{}", node_id.as_simple()),
            analyzer_type,
        )
        .await
}
