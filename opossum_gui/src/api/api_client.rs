use opossum_backend::{
    general::{NodeType, VersionInfo},
    nodes::{ConnectInfo, NewNode, NodeInfo},
    scenery::NewAnalyzerInfo,
    NodeAttr,
};

use super::http_client::HTTPAPIClient;
use uuid::Uuid;

//General api calls

/// Send reqeust to get the version of the opossum backend and the opossum library.
///
/// # Errors
///
/// This function will return an error if
/// - the `VersionInfo` struct cannot be deserialized
pub async fn get_version(client: &HTTPAPIClient) -> Result<VersionInfo, String> {
    client.get::<VersionInfo>("/api/version").await
}
/// Send a request to get all available node types.
///
/// # Errors
///
/// This function will return an error if
/// - the response cannot be deserialized into a vector of [`NodeType`] structs.
pub async fn get_node_types(client: &HTTPAPIClient) -> Result<Vec<NodeType>, String> {
    client.get::<Vec<NodeType>>("/api/node_types").await
}
/// Send a request to check if the bace url is reachable and corresponds to the opossum backend.
///
/// # Errors
///
/// This function will return an error if
/// - the request fails (e.g. the base url is not reachable)
/// - the response cannot be deserialized into a string
pub async fn get_api_welcome(client: &HTTPAPIClient) -> Result<String, String> {
    client.get::<String>("/api/").await
}

// Scenery api calls

/// Send a request to delete the current scenery.
///
/// # Errors
///
/// This function will return an error if
/// - the request fails (e.g. the scenery is not valid)
pub async fn delete_scenery(client: &HTTPAPIClient) -> Result<String, String> {
    client
        .delete::<String, String>("/api/scenery/", String::new())
        .await
}
// pub async fn get_analyzers(&self) -> Result<Vec<AnalyzerType>, String> {
//     self.get::<Vec<AnalyzerType>>("/api/scenery/analyzers")
//         .await
// }

/// Send a request to add an analyzer to the scenery.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`AnalyzerType`] cannot be serialized
pub async fn post_add_analyzer(
    client: &HTTPAPIClient,
    new_analyzer_info: NewAnalyzerInfo,
) -> Result<Uuid, String> {
    client
        .post::<NewAnalyzerInfo, Uuid>("/api/scenery/analyzers", new_analyzer_info)
        .await
}
// pub async fn get_analyzer_at_index(&self, index: usize) -> Result<AnalyzerType, String> {
//     self.get::<AnalyzerType>(&format!("/api/scenery/analyzers/{}", index))
//         .await
// }
// pub async fn delete_analyzer_at_index(&self, index: usize) -> Result<String, String> {
//     self.delete::<String>(&format!("/api/scenery/analyzers/{}", index), index)
//         .await
// }

/// Get all nodes in the current scenery
///
/// # Errors
///
/// This function will return an error if
/// - the request fails (e.g. the scenery is not valid)
/// - the response cannot be deserialized into a vector of [`NodeInfo`] structs
pub async fn get_nodes(client: &HTTPAPIClient) -> Result<Vec<NodeInfo>, String> {
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
    client: &HTTPAPIClient,
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
pub async fn delete_node(client: &HTTPAPIClient, id: Uuid) -> Result<Vec<Uuid>, String> {
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
pub async fn get_node_properties(client: &HTTPAPIClient, uuid: Uuid) -> Result<NodeAttr, String> {
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
    client: &HTTPAPIClient,
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
    client: &HTTPAPIClient,
    connection: ConnectInfo,
) -> Result<ConnectInfo, String> {
    client
        .delete::<ConnectInfo, ConnectInfo>("/api/scenery/connection", connection)
        .await
}
// pub async fn put_node_properties(&self, uuid: Uuid, props: NodeAttr) -> Result<NodeAttr, String> {
//     self.put::<NodeAttr, NodeAttr>(&format!("/api/scenery/nodes/{}", uuid.as_simple().to_string()), props.serialize(serializer)).await
// }
// pub async fn patch_node_properties(&self, uuid: Uuid, props: NodeAttr) -> Result<NodeAttr, String> {
//     self.put::<NodeAttr, NodeAttr>(&format!("/api/scenery/nodes/{}", uuid.as_simple().to_string()), props.serialize(serializer)).await
// }
