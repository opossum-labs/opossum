use opossum_backend::{
    nodes::{ConnectInfo, NewNode, NodeInfo},
    NodeAttr,
};
use uuid::Uuid;

use super::http_client::HTTPClient;

/// Get all nodes in the current scenery
///
/// # Errors
///
/// This function will return an error if
/// - the request fails (e.g. the scenery is not valid)
/// - the response cannot be deserialized into a vector of [`NodeInfo`] structs
pub async fn get_nodes(client: &HTTPClient) -> Result<Vec<NodeInfo>, String> {
    client.get::<Vec<NodeInfo>>("/api/scenery/nodes").await
}
/// Send a request to add a node to the scenery.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`NodeType`] cannot be serialized
/// - the request fails (e.g. the node type is not valid)
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
        .get::<NodeAttr>(&format!("/api/scenery/{}/properties", uuid.as_simple()))
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
pub async fn update_distance(
    client: &HTTPClient,
    connection: ConnectInfo,
) -> Result<ConnectInfo, String> {
    client
        .put::<ConnectInfo, ConnectInfo>("/api/scenery/connection", connection)
        .await
}
// pub async fn put_node_properties(&self, uuid: Uuid, props: NodeAttr) -> Result<NodeAttr, String> {
//     self.put::<NodeAttr, NodeAttr>(&format!("/api/scenery/nodes/{}", uuid.as_simple().to_string()), props.serialize(serializer)).await
// }
// pub async fn patch_node_properties(&self, uuid: Uuid, props: NodeAttr) -> Result<NodeAttr, String> {
//     self.put::<NodeAttr, NodeAttr>(&format!("/api/scenery/nodes/{}", uuid.as_simple().to_string()), props.serialize(serializer)).await
// }
