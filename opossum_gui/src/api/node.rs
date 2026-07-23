use dioxus::html::geometry::euclid::default::Point2D;
use opossum_core::{
    prelude::*,
    types::api_types::{
        AddPortMappingRequest, AnalyzerItemDto, ConnectInfo, ConvertToGroupRequest,
        ConvertToGroupResponse, DeleteNodeResponse, MoveNodesRequest, MoveNodesResponse, NewNode,
        NewRefNode, NodeInfo, NodePortsResponse, NodePropertiesResponse, PortMappingsResponse,
        PortNamesResponse, RemovePortMapResponse, UpdateConnectionRequest, UpdateNodeRequest,
        UpdatePortRequest,
    },
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::HTTP_API_CLIENT;

/// Get all child nodes of the current node group
///
/// # Errors
///
/// This function will return an error if
/// - the request fails (e.g. the node group with the given ID does not exist)
/// - the response cannot be deserialized into a vector of [`NodeInfo`] structs
pub async fn get_nodes(group_id: Uuid) -> Result<Vec<NodeInfo>, String> {
    HTTP_API_CLIENT()
        .get::<Vec<NodeInfo>>(&format!("/api/nodes/{group_id}/children"))
        .await
}
/// Get a list of all connections (edges) of the given node group.
///
/// # Errors
///
/// This function will return an error if
/// - the given `node_id` does not correspond to a group node.
pub async fn get_connections(group_id: Uuid) -> Result<Vec<ConnectInfo>, String> {
    HTTP_API_CLIENT()
        .get::<Vec<ConnectInfo>>(&format!("/api/nodes/{group_id}/connections"))
        .await
}

pub async fn get_port_maps_of_group(group_id: Uuid) -> Result<PortMappingsResponse, String> {
    HTTP_API_CLIENT()
        .get::<PortMappingsResponse>(&format!("/api/nodes/{group_id}/port_mappings"))
        .await
}

pub async fn get_ports_of_group(group_id: Uuid) -> Result<NodePortsResponse, String> {
    HTTP_API_CLIENT()
        .get::<NodePortsResponse>(&format!("/api/nodes/{group_id}/ports"))
        .await
}

pub async fn patch_node_port_config(
    node_id: Uuid,
    port_name: String,
    port_type: PortType,
    req: UpdatePortRequest,
) -> Result<(), String> {
    let port_type_str = match port_type {
        PortType::Input => "Input",
        PortType::Output => "Output",
    };
    HTTP_API_CLIENT()
        .patch::<UpdatePortRequest>(
            &format!("/api/nodes/{node_id}/ports/{port_type_str}/{port_name}"),
            req,
        )
        .await
}
/// Send a request to add a node to a node group.
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
        .post::<NewNode, NodeInfo>(&format!("/api/nodes/{group_id}/children"), new_node_info)
        .await
}

/// Send a request to copy nodes.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`Uuid`] cannot be serialized
/// - the request fails (e.g. the node ide does not exist)
pub async fn post_copy_nodes(nodes: HashSet<Uuid>) -> Result<String, String> {
    HTTP_API_CLIENT()
        .post::<HashSet<Uuid>, String>("/api/operations/copy_nodes", nodes)
        .await
}

/// Pastes the currently copied nodes into `group_id` at `pos`. If `cut` is set, the copied nodes'
/// originals are deleted as part of the same request, so the backend can push a single undo step that
/// reverts both the paste and the delete together - see the backend's `post_paste_nodes` doc comment.
/// The response's last element mirrors what the removed `post_cut_nodes` used to return: the deleted
/// node ids and their former parent graph id, present only when `cut` was set.
pub async fn post_paste_nodes(
    group_id: Uuid,
    pos: Point2D<f64>,
    cut: bool,
) -> Result<
    (
        HashMap<Uuid, Vec<NodeInfo>>,
        Vec<AnalyzerItemDto>,
        HashMap<Uuid, Vec<ConnectInfo>>,
        Option<(
            Vec<Uuid>,
            Vec<Uuid>,
            Vec<(Uuid, ConnectInfo)>,
            Vec<(Uuid, Uuid, String, PortType)>,
        )>,
    ),
    String,
> {
    HTTP_API_CLIENT()
        .post::<(Uuid, (f64, f64), bool), (
            HashMap<Uuid, Vec<NodeInfo>>,
            Vec<AnalyzerItemDto>,
            HashMap<Uuid, Vec<ConnectInfo>>,
            Option<(
                Vec<Uuid>,
                Vec<Uuid>,
                Vec<(Uuid, ConnectInfo)>,
                Vec<(Uuid, Uuid, String, PortType)>,
            )>,
        )>(
            "/api/operations/paste_nodes",
            (group_id, (pos.x, pos.y), cut),
        )
        .await
}

/// Send a request to add a reference node to a node group.
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
        .post::<NewRefNode, NodeInfo>(&format!("/api/nodes/{group_id}/references"), new_ref_info)
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
pub async fn delete_node(id: Uuid) -> Result<DeleteNodeResponse, String> {
    HTTP_API_CLIENT()
        .delete::<String, DeleteNodeResponse>(&format!("/api/nodes/{id}"), String::new())
        .await
}
/// Get the `NodeInfo` of an optical node.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`Uuid`] cannot be serialized or found
pub async fn get_node_info(uuid: Uuid) -> Result<NodeInfo, String> {
    HTTP_API_CLIENT()
        .get_ron::<NodeInfo>(&format!("/api/nodes/{uuid}"))
        .await
}

/// Get the properties of an optical node.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`Uuid`] cannot be serialized or found
/// - the properties cannot be deserialized into the [`NodeInfo`] struct
pub async fn get_node_properties(uuid: Uuid) -> Result<NodePropertiesResponse, String> {
    HTTP_API_CLIENT()
        .get_ron::<NodePropertiesResponse>(&format!("/api/nodes/{uuid}/properties"))
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
        .post::<ConnectInfo, ConnectInfo>(&format!("/api/nodes/{group_id}/connections"), connection)
        .await
}
/// Delete a connection between two nodes.
///
/// # Errors
///
/// This function will return an error if the provided [`ConnectInfo`] does not refer to
/// an existing connection or the server connection fails.
pub async fn delete_connection(connection: ConnectInfo, group_id: Uuid) -> Result<(), String> {
    let source_id = connection.src_uuid();
    let source_port = connection.src_port();
    HTTP_API_CLIENT()
        .delete_no_content(&format!(
            "/api/nodes/{group_id}/connections?src_uuid={source_id}&src_port={source_port}"
        ))
        .await
}
/// Update the physical distance between two nodes.
///
/// # Errors
///
/// This function will return an error if the connection could not be found.
pub async fn update_distance(
    connection: UpdateConnectionRequest,
    group_id: Uuid,
) -> Result<(), String> {
    HTTP_API_CLIENT()
        .patch::<UpdateConnectionRequest>(&format!("/api/nodes/{group_id}/connections"), connection)
        .await
}
/// Update the name of the node with the given `node_id`.
///
/// # Errors
///
/// This function will return an error if the `node_id` was not found.
pub async fn update_node_name(node_id: Uuid, node_name: &str) -> Result<(), String> {
    let update_request = UpdateNodeRequest {
        name: Some(node_name.to_string()),
        ..Default::default()
    };
    HTTP_API_CLIENT()
        .patch::<UpdateNodeRequest>(&format!("/api/nodes/{node_id}"), update_request)
        .await
}

pub async fn get_node_references(node_id: Uuid) -> Result<HashMap<Uuid, Vec<Uuid>>, String> {
    HTTP_API_CLIENT()
        .get::<HashMap<Uuid, Vec<Uuid>>>(&format!("/api/nodes/{node_id}/references"))
        .await
}

/// Update the alignment of the node with the given `node_id`.
///
/// # Errors
/// This function will return an error if the `node_id` was not found or if the alignment cannot be serialized.
pub async fn update_node_alignment(node_id: Uuid, alignment: Isometry) -> Result<(), String> {
    let update_node_request = UpdateNodeRequest {
        alignment: Some(Some(alignment)),
        ..Default::default()
    };
    HTTP_API_CLIENT()
        .patch::<UpdateNodeRequest>(&format!("/api/nodes/{node_id}"), update_node_request)
        .await
}

/// Get the hierarchy of the group with the given `group_id`.
///
/// # Errors
///
/// This function will return an error if the `group_id` was not found.
pub async fn get_group_hierarchy(group_id: Uuid) -> Result<Vec<(Uuid, String)>, String> {
    HTTP_API_CLIENT()
        .get::<Vec<(Uuid, String)>>(&format!("/api/nodes/{group_id}/hierarchy"))
        .await
}

/// Convert the given `nodes` into a subgroup of the specified `group_id`.
///
/// Returns information about the created group node and its connections.
///
/// # Errors
///
/// This function will return an error if the `group_id` was not found
/// or if any of the provided `nodes` are invalid.
pub async fn convert_nodes_to_group(
    nodes: Vec<Uuid>,
    group_id: Uuid,
) -> Result<ConvertToGroupResponse, String> {
    let convert_to_group_request = ConvertToGroupRequest {
        group_id,
        nodes_to_convert: nodes,
    };
    HTTP_API_CLIENT()
        .post::<ConvertToGroupRequest, ConvertToGroupResponse>(
            "/api/operations/convert_to_group",
            convert_to_group_request,
        )
        .await
}

/// Move the given `nodes` from `from_group_id` into `drop_group_id`.
///
/// # Errors
///
/// This function will return an error if either group was not found
/// or if any of the provided `nodes` are invalid.
pub async fn drop_nodes_into_group(
    nodes: Vec<Uuid>,
    from_group_id: Uuid,
    drop_group_id: Uuid,
) -> Result<MoveNodesResponse, String> {
    let move_nodes_request = MoveNodesRequest {
        source_group_id: from_group_id,
        target_group_id: drop_group_id,
        nodes_to_move: nodes.clone(),
    };
    HTTP_API_CLIENT()
        .post::<MoveNodesRequest, MoveNodesResponse>(
            "/api/operations/move_nodes",
            move_nodes_request,
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
) -> Result<(), String> {
    HTTP_API_CLIENT()
        .patch_ron::<Proptype>(
            &format!("/api/nodes/{node_id}/properties/{}", property_key_val.0),
            property_key_val.1,
        )
        .await
}

/// Updates the isometry (position and orientation) of a node.
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
pub async fn update_node_isometry(node_id: Uuid, iso: Option<Isometry>) -> Result<(), String> {
    let update_node_request = UpdateNodeRequest {
        isometry: Some(iso),
        ..Default::default()
    };
    HTTP_API_CLIENT()
        .patch::<UpdateNodeRequest>(&format!("/api/nodes/{node_id}"), update_node_request)
        .await
}

/// Update the inversion state of a node.
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
pub async fn update_node_inversion(node_id: Uuid, inverted: bool) -> Result<(), String> {
    let update_node_request = UpdateNodeRequest {
        inverted: Some(inverted),
        ..Default::default()
    };
    HTTP_API_CLIENT()
        .patch::<UpdateNodeRequest>(&format!("/api/nodes/{node_id}"), update_node_request)
        .await
}

pub async fn add_port_map(
    port_type: PortType,
    group_port_name: String,
    internal_port_name: String,
    internal_node_id: Uuid,
    group_id: Uuid,
) -> Result<PortNamesResponse, String> {
    let add_port_mapping_request = AddPortMappingRequest {
        external_port_name: group_port_name.clone(),
        internal_node_id,
        internal_port_name,
        port_type,
    };
    HTTP_API_CLIENT()
        .post::<AddPortMappingRequest, PortNamesResponse>(
            &format!("/api/nodes/{group_id}/port_mappings"),
            add_port_mapping_request,
        )
        .await
}

pub async fn remove_port_map(
    group_port_name: String,
    group_id: Uuid,
    port_type: PortType,
) -> Result<RemovePortMapResponse, String> {
    let port_type_str = match port_type {
        PortType::Input => "Input",
        PortType::Output => "Output",
    };
    HTTP_API_CLIENT()
        .delete::<(), RemovePortMapResponse>(
            &format!("/api/nodes/{group_id}/port_mappings?external_port_name={group_port_name}&port_type={port_type_str}"),
            (),
        )
        .await
}
