use crate::{
    app_state::AppState, error::BackEndErrorResponse, groups::RemovePortMapQuery,
    utils::update_node_attr,
};
use actix_web::{
    HttpResponse, Responder, delete, get,
    guard::GuardContext,
    http::header,
    patch, post, put,
    web::{self, Json, PathConfig},
};
use nalgebra::Point2;
use opossum_core::{
    core_optics::{NodeAttr, OpticRef, PortType},
    error::OpossumError,
    meter,
    nodes::{NodeReference, create_node_ref},
    prelude::{OpmDocument, OpticNode, PortMap},
    properties::Proptype,
    types::api_types::{ConnectInfo, NewNode, NewRefNode, NodeInfo, UpdateNodeRequest},
    utils::{LockExt, geom_transformation::Isometry},
};
use parking_lot::MutexGuard;
use serde::Deserialize;
use uom::si::length::meter;
use utoipa::IntoParams;
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

#[derive(Debug, Deserialize, IntoParams)]
pub struct DeleteConnectionQuery {
    /// UUID of the source node
    pub src_uuid: Uuid,
    /// Name of the source port
    pub src_port: String,
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
#[patch("/{uuid}")] // URL: /api/nodes/{uuid}
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

/// helper function for checking the ACCEPT header.
fn wants_ron_guard(ctx: &GuardContext<'_>) -> bool {
    if let Some(val) = ctx.head().headers.get(header::ACCEPT)
        && let Ok(s) = val.to_str()
    {
        return s.contains("application/ron");
    }
    false
}

/// Get all nodes of a group node
///
/// Return a list of all nodes of a group node specified by its UUID.
/// - **Note**: If the `nil` UUID is given (00000000-0000-0000-0000-000000000000), all toplevel nodes are returned.
/// - **Note**: This function searches recursively for the UUID in the whole scenery.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"),
    ),
    responses(
        (status = OK, description = "get all nodes of the group", body= Vec<NodeInfo>, content_type="application/json"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found or not a group node", content_type="application/json")
    )
)]
#[get("/{uuid}/nodes")]
async fn get_subnodes(
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
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"),
    ),
    responses(
        (status = OK, description = "all connections of the group", body= Vec<ConnectInfo>, content_type="application/json"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found or not a group node", content_type="application/json")
    )
)]
#[allow(clippy::significant_drop_tightening)]
#[get("/{uuid}/connections")]
pub async fn get_connections(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<ConnectInfo>>, BackEndErrorResponse> {
    let document = data.document.lock();
    let scenery = document.scenery();

    let uuid = path.into_inner();
    let connections = scenery.with_group_node(uuid, opossum_core::nodes::NodeGroup::connections)?;
    let connect_infos = connections
        .iter()
        .map(|c| {
            let is_reference = scenery
                .with_node_attr(c.target_id, |node_attr| {
                    let prop = node_attr.properties();
                    prop.get("reference id").is_ok()
                })
                .unwrap_or(false);
            ConnectInfo::new(
                c.src_id,
                c.src_port.clone(),
                c.target_id,
                c.target_port.clone(),
                c.distance.get::<meter>(),
                is_reference,
            )
        })
        .collect::<Vec<ConnectInfo>>();
    Ok(Json(connect_infos))
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
#[post("/{uuid}/nodes")]
async fn post_subnode(
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
async fn post_subreference(
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
/// Update the GUI position of an optical or analyzer node
// #[utoipa::path(tag = "node",
//     params(
//         ("uuid" = Uuid, Path, description = "UUID of the optical or analyzer node"),
//     ),
//     request_body(content = (f64,f64),
//         description = "updated GUI position",
//         content_type = "application/json",
//         example= "[1.0, 2.0]"
//     ),
//     responses(
//         (status = OK, description = "Node position successfully updated"),
//         (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
//     )
// )]
// #[post("/position/{uuid}")]
// async fn post_node_position(
//     data: web::Data<AppState>,
//     path: web::Path<Uuid>,
//     position: web::Json<(f64, f64)>,
// ) -> Result<(), BackEndErrorResponse> {
//     let uuid = path.into_inner();
//     let position = position.into_inner();
//     let position = Point2::new(position.0, position.1);
//     let mut document = data.document.lock();
//     match document
//         .scenery_mut()
//         .with_node_attr_mut(uuid, |node_attr| node_attr.set_gui_position(Some(position)))
//     {
//         Ok(()) => Ok(()),
//         _ => document.analyzers_mut().get_mut(&uuid).map_or_else(
//             || {
//                 Err(BackEndErrorResponse::new(
//                     404,
//                     "Opossum",
//                     "uuid not found in nodes or analyzers",
//                 ))
//             },
//             |analyzer| {
//                 analyzer.set_gui_position(Some(position));
//                 Ok(())
//             },
//         ),
//     }
// }

/// Update the GUI name of an optical node
// #[utoipa::path(tag = "node",
//     params(
//         ("uuid" = Uuid, Path, description = "name of the optical node"),
//     ),
//     request_body(content = String,
//         description = "updated name of node",
//         content_type = "application/json",
//         example= "Lens 1"
//     ),
//     responses(
//         (status = OK, description = "Node name successfully updated"),
//         (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
//     )
// )]
// #[post("/name/{uuid}")]
// async fn post_node_name(
//     data: web::Data<AppState>,
//     path: web::Path<Uuid>,
//     name: web::Json<String>,
// ) -> Result<Json<HashMap<Uuid, String>>, BackEndErrorResponse> {
//     let uuid: Uuid = path.into_inner();
//     let name = name.into_inner();
//     let mut document = data.document.lock();
//     let mut processed_names = HashMap::<Uuid, String>::new();
//     let scenery = document.scenery_mut();
//     if scenery.node_attr().uuid() == uuid {
//         scenery.node_attr_mut().set_name(&name);
//         processed_names.insert(uuid, name);
//     } else {
//         let nodes_to_rename = scenery.graph().find_all_nodes_referring_to_uuid(uuid)?;
//         for node_uuid in &nodes_to_rename {
//             scenery
//                 .with_node_attr_mut(*node_uuid, |node_attr| {
//                     let name = if node_attr.node_type() == "reference" {
//                         format!("ref ({name})")
//                     } else {
//                         name.clone()
//                     };
//                     node_attr.set_name(&name);
//                     processed_names.insert(*node_uuid, name);
//                 })
//                 .map_err(|_| {
//                     BackEndErrorResponse::new(404, "Opossum", "uuid not found in nodes")
//                 })?;
//         }
//     }
//     drop(document);
//     Ok(Json(processed_names))
// }
/// Update the laser-induced damage threshold (LIDT) of an optical node
// #[utoipa::path(tag = "node",
//     params(
//         ("uuid" = Uuid, Path, description = "lidt of the optical node"),
//     ),
//     request_body(content = String,
//         description = "updated lidt of node in J/cm²",
//         content_type = "application/json",
//         example= "1.56"
//     ),
//     responses(
//         (status = OK, description = "Node LIDT successfully updated"),
//         (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
//     )
// )]
// #[post("/lidt/{uuid}")]
// async fn post_node_lidt(
//     data: web::Data<AppState>,
//     path: web::Path<Uuid>,
//     port_name: web::Json<String>,
//     lidt: web::Json<Fluence>,
// ) -> Result<(), BackEndErrorResponse> {
//     let uuid: Uuid = path.into_inner();
//     let lidt = lidt.into_inner();
//     let port_name = port_name.into_inner();
//     let mut document = data.document.lock();
//     document
//         .scenery_mut()
//         .with_node_attr_mut(uuid, |node_attr| {
//             node_attr
//                 .ports_mut()
//                 .set_lidt(&PortType::Input, &port_name, lidt)
//                 .map_err(|e| BackEndErrorResponse::new(404, "Opossum", &e.to_string()))
//         })
//         .map_err(|_| BackEndErrorResponse::new(404, "Opossum", "uuid not found in nodes"))?
// }

/// Update the alignment isometry of an optical node
// #[utoipa::path(tag = "node",
//     params(
//         ("uuid" = Uuid, Path, description = "alignment isometry of the optical node"),
//     ),
//     request_body(content = String,
//         description = "updated alignment isometry of node",
//         content_type = "application/json",
//     ),
//     responses(
//         (status = OK, description = "Node alignment isometry successfully updated"),
//         (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
//     )
// )]
// #[post("/alignmentisometry/{uuid}")]
// async fn post_node_alignment_isometry(
//     data: web::Data<AppState>,
//     path: web::Path<Uuid>,
//     isometry_from_gui: web::Json<Isometry>,
// ) -> Result<(), BackEndErrorResponse> {
//     let uuid: Uuid = path.into_inner();
//     let isometry = isometry_from_gui.into_inner();
//     let mut document = data.document.lock();
//     document
//         .scenery_mut()
//         .with_node_attr_mut(uuid, |node_attr| node_attr.set_alignment(isometry))
//         .map_err(|_| BackEndErrorResponse::new(404, "Opossum", "uuid not found in nodes"))
// }

/// Update a property of an optical node
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Update a single property of the optical node"),
    ),
    request_body(content = String,
        description = "updated property of node",
        content_type = "application/ron",
        example= "(\"key\", \"value\")"
    ),
    responses(
        (status = OK, description = "Node property successfully updated"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/ron")
    )
)]
#[post("/property/{uuid}")]
async fn post_node_property(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: String,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid: Uuid = path.into_inner();
    let (prop_key, prop_value): (String, Proptype) = match ron::de::from_str(body.as_str()) {
        Ok((key, proptype)) => (key, proptype),
        Err(e) => {
            return Err(BackEndErrorResponse::new(
                400,
                "Opossum",
                &format!("Failed to deserialize property value: {e}"),
            ));
        }
    };
    let mut document = data.document.lock();
    document
        .scenery_mut()
        .with_node_attr_mut(uuid, |node_attr| {
            match node_attr.set_property(prop_key.as_str(), prop_value) {
                Ok(()) => Ok(HttpResponse::Ok()
                    .content_type("application/ron")
                    .body(ron::ser::to_string("").unwrap())),
                Err(e) => Err(BackEndErrorResponse::new(
                    400,
                    "Opossum",
                    e.to_string().as_str(),
                )),
            }
        })
        .map_err(|_| BackEndErrorResponse::new(404, "Opossum", "uuid not found in nodes"))?
}

/// Update the isometry of an optical node
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "isometry of the optical node"),
    ),
    request_body(content = String,
        description = "updated isometry of node",
        content_type = "application/json",
    ),
    responses(
        (status = OK, description = "Node isometry successfully updated"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/isometry/{uuid}")]
async fn post_node_isometry(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    iso: web::Json<Option<Isometry>>,
) -> Result<(), BackEndErrorResponse> {
    let uuid: Uuid = path.into_inner();
    let iso_opt = iso.into_inner();
    let mut document = data.document.lock();
    document
        .scenery_mut()
        .with_node_attr_mut(uuid, |node_attr| node_attr.set_isometry_option(iso_opt))
        .map_err(|_| BackEndErrorResponse::new(404, "Opossum", "uuid not found in nodes"))
}

/// Update the inverted status of an optical node
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "inverted status of the optical node"),
    ),
    request_body(content = String,
        description = "updated inverted status of node",
        content_type = "application/json",
    ),
    responses(
        (status = OK, description = "Node inverted status successfully updated"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/inversion/{uuid}")]
async fn post_node_inversion(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    inverted: web::Json<bool>,
) -> Result<Json<Vec<ConnectInfo>>, BackEndErrorResponse> {
    let uuid: Uuid = path.into_inner();
    let inverted = inverted.into_inner();
    let mut document = data.document.lock();
    let scenery = document.scenery_mut();
    scenery
        .with_node_attr_mut(uuid, |node_attr| node_attr.set_inverted(inverted))
        .map_err(|_| BackEndErrorResponse::new(404, "Opossum", "uuid not found in nodes"))?;
    match document
        .scenery_mut()
        .graph_mut()
        .update_connections_of_single_inverted_node(uuid)
    {
        Ok(()) => {
            let scenery = document.scenery();
            let connect_infos = scenery
                .connections()
                .iter()
                .map(|c| {
                    let is_reference = scenery
                        .with_node_attr(c.target_id, |node_attr| {
                            let prop = node_attr.properties();
                            prop.get("reference id").is_ok()
                        })
                        .unwrap_or(false);
                    ConnectInfo::new(
                        c.src_id,
                        c.src_port.clone(),
                        c.target_id,
                        c.target_port.clone(),
                        c.distance.get::<meter>(),
                        is_reference,
                    )
                })
                .collect::<Vec<ConnectInfo>>();
            drop(document);
            Ok(Json(connect_infos))
        }
        Err(e) => Err(BackEndErrorResponse::new(
            400,
            "Opossum",
            e.to_string().as_str(),
        )),
    }
}

/// Get the port mappings of a group node
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose portmaps should be sent"),
    ),
    responses(
        (status = OK, description = "Node portmaps successfully sent!"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/port_mappings")]
pub async fn get_group_portmap(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<(PortMap, PortMap)>, BackEndErrorResponse> {
    let group_id = path.into_inner();
    let port_maps = data
        .document
        .lock()
        .scenery_mut()
        .with_group_node_mut(group_id, |g| {
            (
                g.graph().port_map(&PortType::Input).clone(),
                g.graph().port_map(&PortType::Output).clone(),
            )
        })?;
    Ok(Json(port_maps))
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
#[delete("/{uuid}/nodes")]
async fn delete_subnode(
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
// Helper function to contain the core logic
fn get_node_attr_from_state(
    uuid: Uuid,
    data: &web::Data<AppState>,
) -> Result<NodeAttr, BackEndErrorResponse> {
    let document = data.document.lock();
    let node_attr = document
        .scenery()
        .node_recursive(uuid)?
        .0
        .optical_ref
        .lock_opm()?
        .node_attr()
        .clone();
    Ok(node_attr)
}

// Helper function to contain the core logic
/// Retrieve the node attributes of a node, referenced by a reference-node
/// To signal the GUI, that the `node_attributes` are readonly when it is a reference, a bool will be sent if it is a reference or not
/// true: node is a reference
/// false: node is original
fn get_referenced_node_attr_from_state(
    mut is_reference: bool,
    uuid: Uuid,
    document: &MutexGuard<'_, OpmDocument>,
) -> Result<(NodeAttr, bool), BackEndErrorResponse> {
    let node_attr = document
        .scenery()
        .node_recursive(uuid)?
        .0
        .optical_ref
        .lock_opm()?
        .node_attr()
        .clone();
    if node_attr.node_type() == "reference" {
        is_reference = true;
        let ref_node_props = node_attr.properties();
        if let Ok(Proptype::Uuid(ref_uuid)) = ref_node_props.get("reference id") {
            get_referenced_node_attr_from_state(is_reference, *ref_uuid, document)
        } else {
            Err(BackEndErrorResponse::new(
                400,
                "Opossum",
                "'reference id' property not found",
            ))
        }
    } else {
        Ok((node_attr, is_reference))
    }
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
/// Get all properties of the specified node in either JSON or RON format.
///
/// Return all properties (`NodeAttr`) of the node specified by its UUID.
/// The format is determined by the `Accept` header.
/// Defaults to `application/json` if the header is missing or doesn't specify
/// `application/ron`.
///
/// # Important
///
/// Due to the fact that numeric properties can have values such as `nan` or `inf` it is possible to read
/// the data as RON. The standard JSON format does **not** support encoding of these values. They are simply
/// returned as `null` values.
///
/// - **Note**: This function only returns `NodeAttr`, even for group nodes.
///   A possible `graph` structure is omitted.
/// - **Note**: This function searches the node recursively in the whole scenery.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the optical node"),
    ),
    responses(
        (status = OK, description = "get all node properties", content(("application/json"),("application/ron"))),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/properties", guard = "wants_ron_guard")]
async fn get_properties_ron(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<impl Responder, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let document = data.document.lock();
    let (node_attr, is_reference) = get_referenced_node_attr_from_state(false, uuid, &document)?;
    drop(document);
    let body = ron::ser::to_string_pretty(
        &(node_attr, is_reference),
        ron::ser::PrettyConfig::new().new_line("\n"),
    )
    .map_err(|e| OpossumError::Other(format!("RON Serialization Error: {e}")))?;

    Ok(HttpResponse::Ok()
        .content_type("application/ron")
        .body(body))
}

#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the node"),
    ),
    responses(
        (status = OK, description = "get the group hierarchy of a node", content(("application/json"))),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "node with UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/hierarchy")]
async fn get_node_hierarchy(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<(Uuid, String)>>, BackEndErrorResponse> {
    let node_id = path.into_inner();
    let document = data.document.lock();
    let scenery = document.scenery();
    let mut group_hierarchy = scenery.get_node_hierarchy_bottom_up(node_id)?;
    drop(document);
    group_hierarchy.reverse();

    Ok(Json(group_hierarchy))
}

#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the optical node"),
    ),
    responses(
        (status = OK, description = "get all node properties", content(("application/json"),("application/ron"))),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/properties")]
async fn get_properties_json(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<NodeAttr>, BackEndErrorResponse> {
    let node_attr = get_node_attr_from_state(path.into_inner(), &data)?;
    Ok(Json(node_attr))
}
/// Modify node properties
///
/// Modify the properties (`NodeAttr`) of a node specified by its UUID.
/// - **Note**: This functino also searches the node recursively in the whole scenery.
#[utoipa::path(tag = "node",
    responses(
        (status = OK, description = "node properties updated", content_type="application/json"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[patch("/{uuid}/properties")]
#[allow(clippy::significant_drop_tightening)]
async fn patch_properties(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    updated_props: Json<serde_json::Value>,
) -> Result<Json<NodeAttr>, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let update_json = updated_props.into_inner();
    let mut document = data.document.lock();
    document
        .scenery_mut()
        .with_node_attr_mut(uuid, |node_attr| {
            update_node_attr(node_attr, &update_json).map_or_else(
                |_| {
                    Err(BackEndErrorResponse::new(
                        404,
                        "Opossum",
                        "uuid not found in nodes",
                    ))
                },
                |attr| {
                    *node_attr = attr;
                    Ok(web::Json(node_attr.clone()))
                },
            )
        })
        .map_err(|_| BackEndErrorResponse::new(404, "Opossum", "uuid not found in nodes"))?
}

/// Connect two nodes
///
/// Connect to optical nodes by the given connection info.
#[utoipa::path(tag = "node",
    responses(
        (status = OK, description = "node connection created", content_type="application/json"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "group UUID not found", content_type="application/json")
    )
)]
#[post("/{uuid}/connection")]
async fn post_connection(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    connect_info: Json<ConnectInfo>,
) -> Result<Json<ConnectInfo>, BackEndErrorResponse> {
    let group_uuid = path.into_inner();
    let mut document = data.document.lock();
    document
        .scenery_mut()
        .with_group_node_mut(group_uuid, |group| {
            group.connect_nodes(
                connect_info.src_uuid(),
                connect_info.src_port(),
                connect_info.target_uuid(),
                connect_info.target_port(),
                meter!(connect_info.distance()),
            )
        })??;
    let is_ref_node = document
        .scenery()
        .with_node_attr(connect_info.target_uuid(), |n| {
            n.properties().get("reference id").is_ok()
        })?;
    let mut connect_info = connect_info.into_inner();
    connect_info.set_is_reference(is_ref_node);
    drop(document);
    Ok(Json(connect_info))
}
/// Disconnect two nodes
///
/// Removes the connection originating from the specified source node and port.
#[utoipa::path(
    tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group containing the connection"),
        DeleteConnectionQuery // <-- Utoipa zaubert daraus automatisch Query-Parameter für Swagger!
    ),
    responses(
        (status = OK, description = "node connection successfully deleted"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "group UUID not found or disconnection failed", content_type="application/json")
    )
)]
#[delete("/{uuid}/connection")]
async fn delete_connection(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    query: web::Query<DeleteConnectionQuery>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let group_uuid = path.into_inner();
    let query = query.into_inner();

    let mut document = data.document.lock();
    document
        .scenery_mut()
        .with_group_node_mut(group_uuid, |group| {
            group.disconnect_nodes(query.src_uuid, &query.src_port)
        })??;
    drop(document);

    // Einfach ein leeres "200 OK" zurückgeben, da die Verbindung nun weg ist.
    Ok(HttpResponse::Ok().finish())
}
/// Update a connection distance
#[utoipa::path(tag = "node",
    responses(
        (status = OK, description = "node connection updated", content_type="application/json"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "group UUID not found", content_type="application/json")
))]
#[put("/{uuid}/connection")]
async fn update_distance(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    connect_info: Json<ConnectInfo>,
) -> Result<Json<ConnectInfo>, BackEndErrorResponse> {
    let group_uuid = path.into_inner();
    let mut document = data.document.lock();
    document
        .scenery_mut()
        .with_group_node_mut(group_uuid, |group| {
            group.update_connection_distance(
                connect_info.src_uuid(),
                connect_info.src_port(),
                meter!(connect_info.distance()),
            )
        })??;
    drop(document);
    Ok(connect_info)
}
/// Map a port of an internal node to a port of the group node, effectively exposing it as an external port of the group.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose port shouldbe mapped to a port of an internal node"),
    ),
    request_body(content = String,
        description = "Node uuid of internal node, tuple of internal port name of node and external port name pof group and the port type",
        content_type = "application/json",
    ),
    responses(
        (status = OK, description = "Node port successfully mapped to group port"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/{uuid}/port_mappings")]
pub async fn post_port_mapping(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    port_map_info: web::Json<(Uuid, (String, String), PortType)>,
) -> Result<Json<(Vec<String>, Vec<String>)>, BackEndErrorResponse> {
    let group_id = path.into_inner();
    let (node_id_to_map, (internal_port_name, external_port_name), port_type) =
        port_map_info.into_inner();

    let ports = data
        .document
        .lock()
        .scenery_mut()
        .with_group_node_mut(group_id, |g| {
            match port_type {
                PortType::Input => {
                    g.map_input_port(node_id_to_map, &internal_port_name, &external_port_name)
                }
                PortType::Output => {
                    g.map_output_port(node_id_to_map, &internal_port_name, &external_port_name)
                }
            }?;
            let ports = g.ports();
            let inputs = ports
                .ports(&PortType::Input)
                .keys()
                .cloned()
                .collect::<Vec<String>>();
            let outputs = ports
                .ports(&PortType::Output)
                .keys()
                .cloned()
                .collect::<Vec<String>>();
            Ok::<(Vec<String>, Vec<String>), BackEndErrorResponse>((inputs, outputs))
        })??;

    Ok(Json(ports))
}
/// Remove a port mapping from a group
#[utoipa::path(
    tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose port-map should be removed"),
        RemovePortMapQuery
    ),
    responses(
        (status = OK, description = "Node port successfully removed from group"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[allow(clippy::significant_drop_tightening)]
#[delete("/{uuid}/port_mappings")]
pub async fn remove_port_map(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    query: web::Query<RemovePortMapQuery>,
) -> Result<Json<(bool, Vec<ConnectInfo>, Uuid)>, BackEndErrorResponse> {
    let group_id = path.into_inner();

    // Entpacke die Query-Parameter
    let RemovePortMapQuery {
        external_port_name,
        port_type,
    } = query.into_inner();

    let mut document = data.document.lock();
    let scenery = document.scenery_mut();

    // get parent of node
    let (_, parent_group) = scenery.node_recursive(group_id)?;

    // get connections
    let connections = scenery.with_group_node_mut(parent_group, |g| {
        let c = g.graph().get_connection_info_of_node(group_id);

        // does not matter if it is a reference, as the connections are just removed
        c.iter()
            .map(|c| ConnectInfo::from_connection_info(c, false))
            .collect::<Vec<ConnectInfo>>()
    })?;

    // remove connections first before removing the mapping
    scenery.with_group_node_mut(parent_group, |g| {
        for c in &connections {
            g.disconnect_nodes(c.src_uuid(), c.src_port())?;
        }
        Ok::<(), BackEndErrorResponse>(())
    })??;

    let port_removed = scenery.with_group_node_mut(group_id, |g| {
        g.remove_mapped_port(&external_port_name, port_type)
    })?;

    Ok(Json((port_removed, connections, parent_group)))
}
pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(get_subnodes);
    cfg.service(post_subnode);
    cfg.service(post_subreference);
    cfg.service(delete_subnode);
    cfg.service(post_node_property);
    cfg.service(get_properties_ron);
    cfg.service(get_properties_json);
    cfg.service(get_node_hierarchy);
    cfg.service(patch_properties);
    cfg.service(patch_node);

    cfg.service(post_port_mapping);
    cfg.service(get_group_portmap);
    cfg.service(remove_port_map);

    cfg.service(post_connection);
    cfg.service(delete_connection);
    cfg.service(get_connections);
    cfg.service(update_distance);

    cfg.app_data(PathConfig::default().error_handler(|err, _req| {
        BackEndErrorResponse::new(400, "parse error", &err.to_string()).into()
    }));
}

#[cfg(test)]
mod test {
    use crate::{app_state::AppState, error::BackEndErrorResponse};
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use uuid::Uuid;

    #[actix_web::test]
    async fn get_node() {
        let app_state = Data::new(AppState::default());
        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(super::get_properties_json),
        )
        .await;
        let req = test::TestRequest::get()
            .uri(&format!("/{}/properties", Uuid::new_v4()))
            .to_request();
        let resp = app.call(req).await.unwrap();
        let e: BackEndErrorResponse = test::read_body_json(resp).await;
        assert_eq!(e.error_response().status(), StatusCode::BAD_REQUEST);
        assert_eq!(e.error_response().category(), "OpticScenery");
    }
}
