use actix_web::{
    HttpResponse, delete, get, patch, post,
    web::{self, Json},
};
use nalgebra::Point2;
use opossum_core::{
    core_optics::OpticRef,
    nodes::{NodeReference, create_node_ref},
    prelude::{OpmDocument, OpticNode, PortType, Proptype},
    types::api_types::{NewNode, NewRefNode, NodeInfo, UpdateNodeRequest},
    utils::LockExt,
};
use parking_lot::MutexGuard;
use uuid::Uuid;

use crate::{app_state::AppState, error::BackEndErrorResponse};

/// Get all nodes of a group node
///
/// Return a list of all nodes of a group node specified by its UUID.
/// - **Note**: This function searches recursively for the UUID in the whole scenery.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"),
    ),
    responses(
        (status = OK, description = "get all nodes of the group", content((Vec<NodeInfo> = "application/json"))),
        (status = BAD_REQUEST, description = "UUID not found or not a group node", content((BackEndErrorResponse = "application/json")))
    )
)]
#[get("/{uuid}/children")]
async fn get_children(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<NodeInfo>>, BackEndErrorResponse> {
    let document = data.document.lock();
    let scenery = document.scenery().clone();
    drop(document);
    let uuid = path.into_inner();
    let nodes_info = scenery.with_group_node(uuid, |g| {
        g.nodes()
            .iter()
            .map(|n| {
                let node = n.optical_ref.lock_opm().unwrap();
                let name = node.name();
                let node_type = node.node_type();
                let inverted = node.inverted();
                let input_ports = node.ports().names(&PortType::Input);
                let output_ports = node.ports().names(&PortType::Output);
                let gui_position = node.gui_position().map(|position| (position.x, position.y));
                drop(node);
                NodeInfo::new(
                    n.uuid(),
                    name,
                    inverted,
                    node_type,
                    input_ports,
                    output_ports,
                    gui_position,
                )
            })
            .collect::<Vec<NodeInfo>>()
    })?;
    Ok(Json(nodes_info))
}
/// Add a new node to a group node
///
/// This function adds a new optical node to a group node specified by its UUID.
/// - The node type as well as the coordinates of the corresponding GUI element must be given.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the optical node"),
    ),
    request_body(content = NewNode,
        description = "type and GUI position of node the optical node to be created",
        content_type = "application/json",
        example ="{\"node_type\": \"dummy\", \"gui_position\": [0.0,0.0]}"
    ),
    responses(
        (status = OK, body= NodeInfo, description = "Node successfully created", content_type="application/json"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "Node of the given type not found, UUID not found, no group node", content_type="application/json")
    )
)]
#[post("/{uuid}/children")]
async fn post_children(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    node_type: web::Json<NewNode>,
) -> Result<Json<NodeInfo>, BackEndErrorResponse> {
    let new_node_info = node_type.into_inner();
    let new_node_ref = create_node_ref(new_node_info.node_type())?;
    let mut node = new_node_ref.optical_ref.lock_opm()?;
    let node_attr = node.node_attr_mut();
    node_attr.set_gui_position(Some(Point2::new(
        new_node_info.gui_position().0,
        new_node_info.gui_position().1,
    )));
    drop(node);
    let mut document = data.document.lock();
    let uuid = path.into_inner();
    let scenery = document.scenery_mut();

    let new_node_uuid =
        scenery.with_group_node_mut(uuid, |g| g.add_node_ref(new_node_ref.clone()))??;

    drop(document);
    let node = new_node_ref.optical_ref.lock_opm()?;
    let gui_position = node.gui_position().map(|position| (position.x, position.y));
    let node_info = NodeInfo::new(
        new_node_uuid,
        node.name(),
        node.inverted(),
        node.node_type(),
        node.ports().names(&PortType::Input),
        node.ports().names(&PortType::Output),
        gui_position,
    );
    drop(node);
    Ok(Json(node_info))
}
/// Update optical node properties
///
/// Modifies the standard properties (name, inversion, isometries, GUI position) of an optical node
/// specified by its UUID. Only the fields provided in the request body will be updated.
#[utoipa::path(
    tag = "node",
    request_body = UpdateNodeRequest,
    responses(
        (status = OK, description = "Node properties successfully updated"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found or invalid data")
    )
)]
#[patch("/{uuid}")]
async fn patch_node(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    update: web::Json<UpdateNodeRequest>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let update = update.into_inner();
    let mut document = data.document.lock();

    let result = document
        .scenery_mut()
        .with_node_attr_mut(uuid, |node_attr| {
            if let Some(name) = update.name {
                node_attr.set_name(&name);
            }
            if let Some(inverted) = update.inverted {
                node_attr.set_inverted(inverted);
            }
            if let Some(iso_opt) = update.isometry {
                node_attr.set_isometry_option(iso_opt);
            }
            if let Some(align) = update.alignment {
                node_attr.set_alignment(align);
            }
            if let Some(gui_pos_opt) = update.gui_position {
                let pos = gui_pos_opt.map(|(x, y)| Point2::new(x, y));
                node_attr.set_gui_position(pos);
            }
        });

    match result {
        Ok(_) => Ok(HttpResponse::Ok().finish()),
        Err(_) => Err(BackEndErrorResponse::new(
            404,
            "Opossum",
            "Node UUID not found",
        )),
    }
}
/// Delete a node
///
/// This function deletes a node. It also deletes reference nodes which refer to this node.
/// A list of UUIDs of the effectively deleted nodes is returned.
#[utoipa::path(tag = "node",
responses(
    (status = OK, body= Vec<Uuid>, description = "UUIDs of the deleted nodes", content_type="application/json"),
    (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
))]
#[delete("/{uuid}")]
async fn delete_node(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<Uuid>>, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let mut document = data.document.lock();
    let scenery = document.scenery_mut();
    let deleted_nodes = scenery.delete_node(uuid)?;
    drop(document);
    Ok(web::Json(deleted_nodes))
}

/// Add a new reference node to a group node
///
/// Adds a new reference node to the specified group node, identified by its UUID (provided in the path).
/// The reference node will refer to another node, specified by its UUID in the request body.
///
/// - **Note**: If the `nil` UUID (`00000000-0000-0000-0000-000000000000`) is provided as the group UUID, the reference node is added to the toplevel group.
/// - The UUID of the node to be referenced, as well as the coordinates of the corresponding GUI element, must be provided.
/// - The function returns information about the newly created reference node.
///
/// # Parameters
/// - `uuid`: UUID of the group node to which the reference node will be added (provided in the path).
/// - `referring_node`: UUID of the node to be referenced (provided in the request body).
///
/// # Returns
/// - On success: Information about the newly created reference node.
/// - On error: An error response if the UUID is not found or the target is not a group
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"),
    ),
    request_body(content = NewRefNode,
        description = "UUID of the node to be referred to and GUI position of the optical node to be created",
        content_type = "application/json",
        example ="{\"referring_node\": \"3fa85f64-5717-4562-b3fc-2c963f66afa6\", \"gui_position\": [0.0,0.0]}"
    ),
    responses(
        (status = OK, body= NodeInfo, description = "Node successfully created", content_type="application/json"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found, no group node", content_type="application/json")
    )
)]
#[post("/{uuid}/references")]
async fn post_reference(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    ref_node_info: web::Json<NewRefNode>,
) -> Result<Json<NodeInfo>, BackEndErrorResponse> {
    let group_uuid = path.into_inner();
    let ref_node_info = ref_node_info.into_inner();

    let mut document = data.document.lock();
    let referring_node =
        get_nested_referenced_node_from_state(ref_node_info.referring_node(), &document)?;
    let mut node_reference = NodeReference::from_node(&referring_node);

    node_reference
        .node_attr_mut()
        .set_gui_position(Some(Point2::new(
            ref_node_info.gui_position().0,
            ref_node_info.gui_position().1,
        )));

    let scenery = document.scenery_mut();
    let new_node_uuid =
        scenery.with_group_node_mut(group_uuid, |g| g.add_node(node_reference.clone()))??;

    drop(document);
    let node_info = NodeInfo::new(
        new_node_uuid,
        node_reference.name(),
        node_reference.inverted(),
        node_reference.node_type(),
        node_reference.ports().names(&PortType::Input),
        node_reference.ports().names(&PortType::Output),
        Some(ref_node_info.gui_position()),
    );
    Ok(Json(node_info))
}
fn get_nested_referenced_node_from_state(
    uuid: Uuid,
    document: &MutexGuard<'_, OpmDocument>,
) -> Result<OpticRef, BackEndErrorResponse> {
    let optic_ref = document.scenery().node_recursive(uuid)?.0;
    let node_attr = optic_ref.optical_ref.lock_opm()?.node_attr().clone();
    if node_attr.node_type() == "reference" {
        let ref_node_props = node_attr.properties();
        if let Ok(Proptype::Uuid(ref_uuid)) = ref_node_props.get("reference id") {
            get_nested_referenced_node_from_state(*ref_uuid, document)
        } else {
            Err(BackEndErrorResponse::new(
                400,
                "Opossum",
                "'reference id' property not found",
            ))
        }
    } else {
        Ok(optic_ref)
    }
}
