use dioxus::html::geometry::euclid::default::Point2D;
use opossum_backend::{
    nodes::{ConnectInfo, NewNode, NewRefNode, NodeInfo},
    Fluence, Isometry, NodeAttr,
};
use serde_json::Value;
use uuid::Uuid;

use super::http_client::HTTPClient;

/// Get all nodes in the current scenery
///
/// # Errors
///
/// This function will return an error if
/// - the request fails (e.g. the scenery is not valid)
/// - the response cannot be deserialized into a vector of [`NodeInfo`] structs
pub async fn get_nodes(client: &HTTPClient, group_id: Uuid) -> Result<Vec<NodeInfo>, String> {
    client
        .get::<Vec<NodeInfo>>(&format!("/api/scenery/{}/nodes", group_id.as_simple()))
        .await
}
/// Get a list of all connections (edges) of the given node group.
/// If the `node_id` is `Uuid::nil()` the connections of the toplevel group
/// are returned.
///
/// # Errors
///
/// This function will return an error if
/// - the given `node_id` is not `Uuid::nil()` and does not correspond to a (sub-)group of the scenery.
pub async fn get_connections(
    client: &HTTPClient,
    group_id: Uuid,
) -> Result<Vec<ConnectInfo>, String> {
    client
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
pub async fn post_add_node(
    client: &HTTPClient,
    new_node_info: NewNode,
    group_id: Uuid,
) -> Result<NodeInfo, String> {
    client
        .post::<NewNode, NodeInfo>(
            &format!("/api/scenery/{}/nodes", group_id.as_simple()),
            new_node_info,
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
    client: &HTTPClient,
    new_ref_info: NewRefNode,
    group_id: Uuid,
) -> Result<NodeInfo, String> {
    client
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
pub async fn delete_node(client: &HTTPClient, id: Uuid) -> Result<Vec<Uuid>, String> {
    client
        .delete::<String, Vec<Uuid>>(
            &format!("/api/scenery/{}/nodes", id.as_simple()),
            String::new(),
        )
        .await
}
/// Get the properties of a node.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`Uuid`] cannot be serialized or found
/// - the properties cannot be deserialized into the [`NodeAttr`] struct
pub async fn get_node_properties(client: &HTTPClient, uuid: Uuid) -> Result<NodeAttr, String> {
    client
        .get_ron::<NodeAttr>(&format!("/api/scenery/{}/properties", uuid.as_simple()))
        .await
}
/// Connect two nodes.
///
/// # Errors
///
/// This function will return an error if the provided [`ConnectInfo`] cannot be serialized or if the request fails.
pub async fn post_add_connection(
    client: &HTTPClient,
    connection: ConnectInfo,
) -> Result<ConnectInfo, String> {
    client
        .post::<ConnectInfo, ConnectInfo>("/api/scenery/connection", connection)
        .await
}
/// Delete a connection between two nodes.
///
/// # Errors
///
/// This function will return an error if the provided [`ConnectInfo`] cannot be serialized or if the request fails.
pub async fn delete_connection(
    client: &HTTPClient,
    connection: ConnectInfo,
) -> Result<ConnectInfo, String> {
    client
        .delete::<ConnectInfo, ConnectInfo>("/api/scenery/connection", connection)
        .await
}
/// Update the physical distance between two nodes.
///
/// # Errors
///
/// This function will return an error if the connection could not be found.
pub async fn update_distance(
    client: &HTTPClient,
    connection: ConnectInfo,
) -> Result<ConnectInfo, String> {
    client
        .put::<ConnectInfo, ConnectInfo>("/api/scenery/connection", connection)
        .await
}
/// Update the GUI position coordinates of the node with the given `node_id`.
///
/// # Errors
///
/// This function will return an error if the `node_id` was not found.
pub async fn update_gui_position(
    client: &HTTPClient,
    node_id: Uuid,
    gui_position: Point2D<f64>,
) -> Result<String, String> {
    let position = (gui_position.x, gui_position.y);
    client
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
    client: &HTTPClient,
    node_id: Uuid,
    node_name: String,
) -> Result<String, String> {
    client
        .post::<String, String>(
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
pub async fn update_node_lidt(
    client: &HTTPClient,
    node_id: Uuid,
    node_lidt: Fluence,
) -> Result<String, String> {
    client
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
pub async fn update_node_alignment(
    client: &HTTPClient,
    node_id: Uuid,
    alignment: Isometry,
) -> Result<String, String> {
    client
        .post::<Isometry, String>(
            &format!("/api/scenery/alignmentisometry/{}", node_id.as_simple()),
            alignment,
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
    client: &HTTPClient,
    node_id: Uuid,
    property_key_val: (String, Value),
) -> Result<String, String> {
    client
        .post::<(String, Value), String>(
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
pub async fn update_node_isometry(
    client: &HTTPClient,
    node_id: Uuid,
    iso: Isometry,
) -> Result<String, String> {
    client
        .post::<Isometry, String>(
            &format!("/api/scenery/isometry/{}", node_id.as_simple()),
            iso,
        )
        .await
}
