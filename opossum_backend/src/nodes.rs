use std::collections::{HashMap, HashSet};

use crate::{
    app_state::{AppState, NodeCacheItem},
    error::BackEndErrorResponse,
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
    OpticRef,
    analyzers::AnalyzerType,
    error::OpossumError,
    meter,
    nodes::{
        ConnectionInfo, NodeAttr, NodeGroup, NodeReference, create_node_ref,
        fluence_detector::Fluence,
    },
    opm_document::AnalyzerInfo,
    optic_ports::PortType,
    prelude::{OpmDocument, OpticNode, PortMap},
    properties::Proptype,
    types::api_types::{ConnectInfo, NewNode, NewRefNode, NodeInfo},
    utils::{LockExt, geom_transformation::Isometry},
};
use parking_lot::MutexGuard;
use uom::si::length::meter;
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

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
        (status = OK, description = "get all nodes of the group", content_type="application/json"),
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

fn upper_left_corner_of_nodes(
    nodes: &[NodeCacheItem],
) -> Result<Point2<f64>, BackEndErrorResponse> {
    let mut corner = Point2::new(f64::INFINITY, f64::INFINITY);

    for node in nodes {
        let pos = match node {
            NodeCacheItem::Optical(optical_node) => {
                let node = optical_node.optical_ref.lock_opm()?;
                node.gui_position().unwrap_or_else(Point2::origin)
            }
            NodeCacheItem::Analyzer(analyzer) => {
                analyzer.gui_position().unwrap_or_else(Point2::origin)
            }
        };

        corner.x = corner.x.min(pos.x);
        corner.y = corner.y.min(pos.y);
    }

    Ok(corner)
}

pub fn copy_from_optic_ref(
    scenery: &NodeGroup,
    optic_ref: &OpticRef,
) -> Result<(OpticRef, Uuid), BackEndErrorResponse> {
    let node_to_copy_from = optic_ref.optical_ref.lock_opm()?;
    let old_node_id = node_to_copy_from.node_attr().uuid();

    let new_node_ref = create_node_ref(&node_to_copy_from.node_type())?;
    let mut node = new_node_ref.optical_ref.lock_opm()?;

    if let Ok(Proptype::Uuid(ref_uuid)) = node_to_copy_from
        .node_attr()
        .properties()
        .get("reference id")
        && let Ok(ref_node) = node.as_refnode_mut()
    {
        let (referenced_node, _) = scenery.node_recursive(*ref_uuid)?;
        ref_node.assign_reference(&referenced_node);
    }

    let node_attr = node.node_attr_mut();
    node_attr.replace_from_node_attr(node_to_copy_from.node_attr());

    drop(node_to_copy_from);
    drop(node);

    Ok((new_node_ref, old_node_id))
}

pub fn get_shifted_pos_of_ref(
    optic_ref: &OpticRef,
    shift: Point2<f64>,
) -> Result<(f64, f64), BackEndErrorResponse> {
    let node_to_copy_from = optic_ref.optical_ref.lock_opm()?;
    let old_pos = node_to_copy_from
        .gui_position()
        .unwrap_or_else(Point2::origin);
    let new_pos = (old_pos.x + shift.x, old_pos.y + shift.y);
    drop(node_to_copy_from);
    Ok(new_pos)
}

pub fn copy_optical_node(
    scenery: &mut NodeGroup,
    group_id: Uuid,
    group_id_to_copy: Uuid,
    shift: Point2<f64>,
    optic_ref: &OpticRef,
    node_id_link: &mut HashMap<Uuid, Uuid>,
    grouped_connect_info: &mut HashMap<Uuid, HashMap<Uuid, Vec<ConnectionInfo>>>,
) -> Result<NodeInfo, BackEndErrorResponse> {
    let (new_node_ref, old_node_id) = copy_from_optic_ref(scenery, optic_ref)?;

    let new_pos = get_shifted_pos_of_ref(optic_ref, shift)?;

    let mut node = new_node_ref.optical_ref.lock_opm()?;
    let node_attr = node.node_attr_mut();
    node_attr.set_gui_position(Some(Point2::new(new_pos.0, new_pos.1)));

    drop(node);

    let new_node_uuid =
        scenery.with_group_node_mut(group_id, |g| g.add_node_ref(new_node_ref.clone()))??;

    node_id_link.insert(old_node_id, new_node_uuid);

    scenery.with_group_node(group_id_to_copy, |group: &NodeGroup| {
        let connect = group
            .graph()
            .clone()
            .get_outgoing_connection_info_of_node(old_node_id);

        if let Some(c_info_map) = grouped_connect_info.get_mut(&group_id) {
            c_info_map.insert(old_node_id, connect);
        }
    })?;

    let node = new_node_ref.optical_ref.lock_opm()?;
    Ok(NodeInfo::new(
        new_node_uuid,
        node.name(),
        node.inverted(),
        node.node_type(),
        node.ports().names(&PortType::Input),
        node.ports().names(&PortType::Output),
        Some(new_pos),
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn copy_optical_nodes_recursive(
    scenery: &mut NodeGroup,
    group_id_to_insert: Uuid,
    group_id_to_copy: Uuid,
    shift: Point2<f64>,
    copied_optical_nodes: &[OpticRef],
    node_id_link: &mut HashMap<Uuid, Uuid>,
    grouped_connect_info: &mut HashMap<Uuid, HashMap<Uuid, Vec<ConnectionInfo>>>,
    grouped_node_infos: &mut HashMap<Uuid, Vec<NodeInfo>>,
    input_port_maps: &mut HashMap<Uuid, PortMap>,
    output_port_maps: &mut HashMap<Uuid, PortMap>,
) -> Result<(), BackEndErrorResponse> {
    let mut optical_nodes = Vec::new();
    grouped_connect_info.insert(
        group_id_to_insert,
        HashMap::<Uuid, Vec<ConnectionInfo>>::new(),
    );
    for node in copied_optical_nodes {
        let node_id = node.uuid();
        let group_nodes_opt = node.optical_ref.lock_opm()?.as_group().map_or_else(
            |_| None,
            |group| {
                input_port_maps.insert(node_id, group.graph().port_map(&PortType::Input).clone());
                output_port_maps.insert(node_id, group.graph().port_map(&PortType::Output).clone());
                Some(
                    group
                        .nodes()
                        .iter()
                        .copied()
                        .cloned()
                        .collect::<Vec<OpticRef>>(),
                )
            },
        );
        let copied_node = copy_optical_node(
            scenery,
            group_id_to_insert,
            group_id_to_copy,
            shift,
            node,
            node_id_link,
            grouped_connect_info,
        )?;

        let copied_node_id = copied_node.uuid();
        optical_nodes.push(copied_node);

        if let Some(nodes_in_group) = group_nodes_opt {
            copy_optical_nodes_recursive(
                scenery,
                copied_node_id,
                node.uuid(),
                Point2::origin(),
                &nodes_in_group,
                node_id_link,
                grouped_connect_info,
                grouped_node_infos,
                input_port_maps,
                output_port_maps,
            )?;
        }
    }
    grouped_node_infos.insert(group_id_to_insert, optical_nodes);
    Ok(())
}

fn copy_analyzer(
    data: &web::Data<AppState>,
    shift: Point2<f64>,
    analyzer: &AnalyzerInfo,
) -> AnalyzerInfo {
    let old_pos = analyzer.gui_position().unwrap();

    let new_pos = Point2::new(old_pos.x + shift.x, old_pos.y + shift.y);

    let new_analyzer = AnalyzerInfo::new(analyzer.analyzer_type().clone(), Uuid::new_v4(), new_pos);

    let mut document = data.document.lock();
    document.add_analyzer_info(&new_analyzer);

    new_analyzer
}

fn remap_connections(
    connections: &mut HashMap<Uuid, Vec<ConnectionInfo>>,
    node_id_link: &HashMap<Uuid, Uuid>,
) {
    for connect in connections.values_mut() {
        connect.retain(|c| {
            node_id_link.contains_key(&c.src_id) && node_id_link.contains_key(&c.target_id)
        });

        for c in connect {
            if let Some(id) = node_id_link.get(&c.src_id) {
                c.src_id = *id;
            }
            if let Some(id) = node_id_link.get(&c.target_id) {
                c.target_id = *id;
            }
        }
    }
}

fn set_copied_connections(
    scenery: &mut NodeGroup,
    group_id: Uuid,
    connections: &HashMap<Uuid, Vec<ConnectionInfo>>,
) -> Result<Vec<ConnectInfo>, BackEndErrorResponse> {
    let mut result = Vec::new();

    for conns in connections.values() {
        let enriched: Vec<_> = conns
            .iter()
            .map(|c| {
                let is_reference = scenery
                    .with_node_attr(c.target_id, |attr| {
                        attr.properties().get("reference id").is_ok()
                    })
                    .unwrap_or(false);

                (c, is_reference)
            })
            .collect();

        scenery
            .with_group_node_mut(group_id, |group| -> Result<(), BackEndErrorResponse> {
                for (c, is_reference) in enriched {
                    group.connect_nodes(
                        c.src_id,
                        &c.src_port,
                        c.target_id,
                        &c.target_port,
                        c.distance,
                    )?;

                    result.push(ConnectInfo::new(
                        c.src_id,
                        c.src_port.clone(),
                        c.target_id,
                        c.target_port.clone(),
                        c.distance.value,
                        is_reference,
                    ));
                }
                Ok(())
            })
            .map_err(|e| {
                BackEndErrorResponse::new(404, "Opossum", &format!("Could not paste nodes: {e}"))
            })??;
    }

    Ok(result)
}

#[allow(clippy::significant_drop_tightening)]
fn resolve_references(
    scenery: &mut NodeGroup,
    node_id_link: &HashMap<Uuid, Uuid>,
) -> Result<(), BackEndErrorResponse> {
    for new_id in node_id_link.values() {
        let old_ref_id_opt: Option<Uuid> = scenery
            .with_node_attr(*new_id, |attr| {
                match attr.properties().get("reference id") {
                    Ok(Proptype::Uuid(uuid)) => Some(*uuid),
                    _ => None,
                }
            })
            .ok()
            .flatten();

        let new_ref_id_opt = old_ref_id_opt
            .map(|old_ref_id| node_id_link.get(&old_ref_id).copied().unwrap_or(old_ref_id));

        let referenced_node_opt = new_ref_id_opt.map_or_else(
            || None,
            |new_ref_id| {
                scenery
                    .node_recursive(new_ref_id)
                    .ok()
                    .map(|(node, _)| node)
            },
        );

        if let Some(referenced_node) = referenced_node_opt {
            scenery.with_node_mut(*new_id, |node| {
                if let Ok(ref_node) = node.as_refnode_mut() {
                    ref_node.assign_reference(&referenced_node);
                }
            })?;
        }
    }

    Ok(())
}

/// Paste copied nodes
///
/// This function sends already copied nodes to the frontend
#[utoipa::path(tag = "node",
    request_body(content = (Uuid, (f64, f64)),
        description = "Uuid of the group node to be pasted in and the position at which the node should be pasted",
        content_type = "application/json",
    ),
    responses(
        (status = OK, body= NodeInfo, description = "Node successfully pasted", content_type="application/json"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[allow(clippy::significant_drop_tightening)]
#[post("/nodes_paste")]
async fn post_paste_nodes(
    data: web::Data<AppState>,
    node_paste_info: web::Json<(Uuid, (f64, f64))>,
) -> Result<
    Json<(
        HashMap<Uuid, Vec<NodeInfo>>,
        Vec<AnalyzerInfo>,
        HashMap<Uuid, Vec<ConnectInfo>>,
    )>,
    BackEndErrorResponse,
> {
    let (group_id, node_pos) = node_paste_info.into_inner();
    let paste_in_scenery = data.document.lock().scenery().node_attr().uuid() == group_id;

    let copied_nodes = data.node_copy_cache.lock();
    let min_pos = upper_left_corner_of_nodes(&copied_nodes)?;
    drop(copied_nodes);
    let shift = Point2::new(node_pos.0 - min_pos.x, node_pos.1 - min_pos.y);

    let mut copied_optical_nodes = Vec::<OpticRef>::new();
    let mut copied_analyzer_nodes = Vec::<AnalyzerInfo>::new();

    for cache in data.node_copy_cache.lock().iter() {
        match cache {
            NodeCacheItem::Optical(optic_ref) => copied_optical_nodes.push(optic_ref.clone()),
            NodeCacheItem::Analyzer(analyzer_info) => {
                copied_analyzer_nodes.push(analyzer_info.clone());
            }
        }
    }

    let mut analyzers = Vec::new();
    if paste_in_scenery {
        for analyzer in &copied_analyzer_nodes {
            analyzers.push(copy_analyzer(&data, shift, analyzer));
        }
    }

    let mut document = data.document.lock();
    let scenery = document.scenery_mut();
    let mut grouped_node_infos = HashMap::<Uuid, Vec<NodeInfo>>::new();
    let mut grouped_connect_info = HashMap::<Uuid, Vec<ConnectInfo>>::new();
    let mut grouped_connections = HashMap::<Uuid, HashMap<Uuid, Vec<ConnectionInfo>>>::new();
    let mut node_id_link = HashMap::<Uuid, Uuid>::new();
    let mut input_port_maps = HashMap::<Uuid, PortMap>::new();
    let mut output_port_maps = HashMap::<Uuid, PortMap>::new();
    copy_optical_nodes_recursive(
        scenery,
        group_id,
        group_id,
        shift,
        &copied_optical_nodes,
        &mut node_id_link,
        &mut grouped_connections,
        &mut grouped_node_infos,
        &mut input_port_maps,
        &mut output_port_maps,
    )?;

    resolve_references(scenery, &node_id_link)?;

    reconfigure_ports(
        scenery,
        &input_port_maps,
        &output_port_maps,
        &node_id_link,
        &mut grouped_node_infos,
    )?;

    for (g_id, connections) in &mut grouped_connections {
        remap_connections(connections, &node_id_link);
        let connect_info = set_copied_connections(scenery, *g_id, connections)?;
        grouped_connect_info.insert(*g_id, connect_info);
    }

    Ok(Json((grouped_node_infos, analyzers, grouped_connect_info)))
}

fn reconfigure_ports(
    scenery: &mut NodeGroup,
    input_port_maps: &HashMap<Uuid, PortMap>,
    output_port_maps: &HashMap<Uuid, PortMap>,
    node_id_link: &HashMap<Uuid, Uuid>,
    grouped_node_infos: &mut HashMap<Uuid, Vec<NodeInfo>>,
) -> Result<(), BackEndErrorResponse> {
    //output port maps
    for (old_group_id, output_port_map) in output_port_maps {
        for (external_port_name, (input_node, internal_port_name)) in output_port_map {
            if let (Some(new_group_id), Some(new_mapped_node_id)) =
                (node_id_link.get(old_group_id), node_id_link.get(input_node))
            {
                scenery.with_group_node_mut(*new_group_id, |new_group| {
                    new_group.map_output_port(
                        *new_mapped_node_id,
                        internal_port_name,
                        external_port_name,
                    )?;
                    Ok::<(), BackEndErrorResponse>(())
                })??;
            }
        }
    }

    //input port maps
    for (old_group_id, input_port_map) in input_port_maps {
        for (external_port_name, (input_node, internal_port_name)) in input_port_map {
            if let (Some(new_group_id), Some(new_mapped_node_id)) =
                (node_id_link.get(old_group_id), node_id_link.get(input_node))
            {
                scenery.with_group_node_mut(*new_group_id, |new_group| {
                    new_group.map_input_port(
                        *new_mapped_node_id,
                        internal_port_name,
                        external_port_name,
                    )?;
                    Ok::<(), BackEndErrorResponse>(())
                })??;
            }
        }
    }

    let inverted_node_link: HashMap<Uuid, Uuid> =
        node_id_link.iter().map(|(k, v)| (*v, *k)).collect();

    //set ports
    for node_info in grouped_node_infos.values_mut() {
        for n in node_info {
            if n.node_type() == "group"
                && let Some(old_node_id) = inverted_node_link.get(&n.uuid())
            {
                scenery.with_group_node(*old_node_id, |g| {
                    n.set_input_ports(g.ports().ports(&PortType::Input).keys().cloned().collect());
                    n.set_output_ports(
                        g.ports().ports(&PortType::Output).keys().cloned().collect(),
                    );
                })?;
            }
        }
    }
    Ok(())
}

/// Copy existing nodes
///
/// This function copies a single or multiple already existing nodes
#[utoipa::path(tag = "node",
    request_body(content = HashSet<Uuid>,
        description = "List of Uuids of the nodes to be copied",
        content_type = "application/json",
    ),
    responses(
        (status = OK, body= NodeInfo, description = "Node successfully created", content_type="application/json"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/nodes_copy")]
async fn post_copy_nodes(
    data: web::Data<AppState>,
    node_id: web::Json<HashSet<Uuid>>,
) -> Result<(), BackEndErrorResponse> {
    let mut all_nodes_found = true;
    let node_ids_to_copy = node_id.into_inner();
    //get optic ref of nde that should be copied
    let document = data.document.lock();
    let mut copied_nodes_set = data.node_copy_cache.lock();
    copied_nodes_set.clear();
    for id in &node_ids_to_copy {
        if let Ok((node_ref_to_copy, _)) = document.scenery().node_recursive(*id) {
            copied_nodes_set.push(NodeCacheItem::Optical(node_ref_to_copy));
        } else if let Some(analyzer) = document.analyzers().get(id).cloned() {
            copied_nodes_set.push(NodeCacheItem::Analyzer(analyzer));
        } else {
            all_nodes_found = false;
        }
    }
    drop(copied_nodes_set);
    drop(document);
    if all_nodes_found {
        Ok(())
    } else {
        Err(BackEndErrorResponse::new(
            404,
            "Opossum",
            "Some nodes could not be copied as they were not found in the document",
        ))
    }
}

/// Add a new node to a group node
///
/// This function adds a new optical node to a group node specified by its UUID.
/// - **Note**: If the `nil` UUID is given (`00000000-0000-0000-0000-000000000000`), the node is added to the toplevel group.
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
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the optical or analyzer node"),
    ),
    request_body(content = (f64,f64),
        description = "updated GUI position",
        content_type = "application/json",
        example= "[1.0, 2.0]"
    ),
    responses(
        (status = OK, description = "Node position successfully updated"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/position/{uuid}")]
async fn post_node_position(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    position: web::Json<(f64, f64)>,
) -> Result<(), BackEndErrorResponse> {
    let uuid = path.into_inner();
    let position = position.into_inner();
    let position = Point2::new(position.0, position.1);
    let mut document = data.document.lock();
    match document
        .scenery_mut()
        .with_node_attr_mut(uuid, |node_attr| node_attr.set_gui_position(Some(position)))
    {
        Ok(()) => Ok(()),
        _ => document.analyzers_mut().get_mut(&uuid).map_or_else(
            || {
                Err(BackEndErrorResponse::new(
                    404,
                    "Opossum",
                    "uuid not found in nodes or analyzers",
                ))
            },
            |analyzer| {
                analyzer.set_gui_position(Some(position));
                Ok(())
            },
        ),
    }
}

/// Update the GUI name of an optical node
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "name of the optical node"),
    ),
    request_body(content = String,
        description = "updated name of node",
        content_type = "application/json",
        example= "Lens 1"
    ),
    responses(
        (status = OK, description = "Node name successfully updated"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/name/{uuid}")]
async fn post_node_name(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    name: web::Json<String>,
) -> Result<Json<HashMap<Uuid, String>>, BackEndErrorResponse> {
    let uuid: Uuid = path.into_inner();
    let name = name.into_inner();
    let mut document = data.document.lock();
    let mut processed_names = HashMap::<Uuid, String>::new();
    let scenery = document.scenery_mut();
    if scenery.node_attr().uuid() == uuid {
        scenery.node_attr_mut().set_name(&name);
        processed_names.insert(uuid, name);
    } else {
        let nodes_to_rename = scenery.graph().find_all_nodes_referring_to_uuid(uuid)?;
        for node_uuid in &nodes_to_rename {
            scenery
                .with_node_attr_mut(*node_uuid, |node_attr| {
                    let name = if node_attr.node_type() == "reference" {
                        format!("ref ({name})")
                    } else {
                        name.clone()
                    };
                    node_attr.set_name(&name);
                    processed_names.insert(*node_uuid, name);
                })
                .map_err(|_| {
                    BackEndErrorResponse::new(404, "Opossum", "uuid not found in nodes")
                })?;
        }
    }
    drop(document);
    Ok(Json(processed_names))
}
/// Update the laser-induced damage threshold (LIDT) of an optical node
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "lidt of the optical node"),
    ),
    request_body(content = String,
        description = "updated lidt of node in J/cm²",
        content_type = "application/json",
        example= "1.56"
    ),
    responses(
        (status = OK, description = "Node LIDT successfully updated"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/lidt/{uuid}")]
async fn post_node_lidt(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    lidt: web::Json<Fluence>,
) -> Result<(), BackEndErrorResponse> {
    let uuid: Uuid = path.into_inner();
    let lidt = lidt.into_inner();
    let mut document = data.document.lock();
    document
        .scenery_mut()
        .with_node_attr_mut(uuid, |node_attr| {
            node_attr
                .set_lidt(&lidt)
                .map_err(|e| BackEndErrorResponse::new(404, "Opossum", &e.to_string()))
        })
        .map_err(|_| BackEndErrorResponse::new(404, "Opossum", "uuid not found in nodes"))?
}

/// Update the alignment isometry of an optical node
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "alignment isometry of the optical node"),
    ),
    request_body(content = String,
        description = "updated alignment isometry of node",
        content_type = "application/json",
    ),
    responses(
        (status = OK, description = "Node alignment isometry successfully updated"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/alignmentisometry/{uuid}")]
async fn post_node_alignment_isometry(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    isometry_from_gui: web::Json<Isometry>,
) -> Result<(), BackEndErrorResponse> {
    let uuid: Uuid = path.into_inner();
    let isometry = isometry_from_gui.into_inner();
    let mut document = data.document.lock();
    document
        .scenery_mut()
        .with_node_attr_mut(uuid, |node_attr| node_attr.set_alignment(isometry))
        .map_err(|_| BackEndErrorResponse::new(404, "Opossum", "uuid not found in nodes"))
}

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

// Helper function to contain the core logic
fn get_node_analyzer_attr_from_state(
    uuid: Uuid,
    data: &web::Data<AppState>,
) -> Result<AnalyzerInfo, BackEndErrorResponse> {
    let document = data.document.lock().clone();
    let analyzer_info = document
        .analyzers()
        .get(&uuid)
        .ok_or_else(|| BackEndErrorResponse::new(404, "Opossum", "UUID not found in analyzers"))?
        .clone();
    Ok(analyzer_info)
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
/// Get all info of the specified analyzer node in either JSON or RON format.
///
/// Return all info (`AnalyzerInfo`) of the analyzer node specified by its UUID.
/// The format is determined by the `Accept` header.
/// Defaults to `application/json` if the header is missing or doesn't specify
/// `application/ron`.
///
/// # Important
///
/// Due to the fact that numeric properties can have values such as `nan` or `inf` it is possible to read
/// the data as RON. The standard JSON format does **not** support encoding of these values. They are simply
/// returned as `null` values.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the analyzer node"),
    ),
    responses(
        (status = OK, description = "get all analyzer information", content(("application/json"),("application/ron"))),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/analyzer_info", guard = "wants_ron_guard")]
async fn get_analyzer_info_ron(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<impl Responder, BackEndErrorResponse> {
    let analyzer_info = get_node_analyzer_attr_from_state(path.into_inner(), &data)?;

    let body =
        ron::ser::to_string_pretty(&analyzer_info, ron::ser::PrettyConfig::new().new_line("\n"))
            .map_err(|e| OpossumError::Other(format!("RON Serialization Error: {e}")))?;

    Ok(HttpResponse::Ok()
        .content_type("application/ron")
        .body(body))
}
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the analyzer node"),
    ),
    responses(
        (status = OK, description = "get all analyzer information", content(("application/json"),("application/ron"))),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/analyzer_info")]
async fn get_analyzer_info_json(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<AnalyzerInfo>, BackEndErrorResponse> {
    let analyzer_info = get_node_analyzer_attr_from_state(path.into_inner(), &data)?;
    Ok(Json(analyzer_info))
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
#[utoipa::path(tag = "node",
    responses(
        (status = OK, description = "node connection deleted", content_type="application/json"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "group UUID not found", content_type="application/json")
))]
#[delete("/{uuid}/connection")]
async fn delete_connection(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    connect_info: Json<ConnectInfo>,
) -> Result<Json<ConnectInfo>, BackEndErrorResponse> {
    let group_uuid = path.into_inner();
    let mut document = data.document.lock();
    document
        .scenery_mut()
        .with_group_node_mut(group_uuid, |group| {
            group.disconnect_nodes(connect_info.src_uuid(), connect_info.src_port())
        })??;
    drop(document);
    Ok(connect_info)
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

/// Update the analyzer config of an analyzer node
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Update an analyzer config of the analyzer node"),
    ),
    request_body(content = String,
        description = "updated config of analyzer",
        content_type = "application/ron",
        example= "\"analyzer_type\""
    ),
    responses(
        (status = OK, description = "Analyzer config successfully updated"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/ron")
    )
)]
#[post("/analyzer/{uuid}")]
async fn post_analyzer_config(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: String,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid: Uuid = path.into_inner();
    let analyzer_type: AnalyzerType = match ron::de::from_str(body.as_str()) {
        Ok(analyzer_type) => analyzer_type,
        Err(e) => {
            return Err(BackEndErrorResponse::new(
                400,
                "Opossum",
                &format!("Failed to deserialize property value: {e}"),
            ));
        }
    };
    let mut document = data.document.lock();
    if let Some(analyzer_info) = document.analyzer_mut(uuid) {
        analyzer_info.set_analyzer_type(&analyzer_type);
        drop(document);
    } else {
        return Err(BackEndErrorResponse::new(
            404,
            "Opossum",
            "uuid not found in analyzers",
        ));
    }
    Ok(HttpResponse::Ok()
        .content_type("application/ron")
        .body(ron::ser::to_string("").unwrap()))
}

pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(get_subnodes);
    cfg.service(post_subnode);
    cfg.service(post_subreference);
    cfg.service(delete_subnode);
    cfg.service(post_node_position);
    cfg.service(post_node_name);
    cfg.service(post_copy_nodes);
    cfg.service(post_paste_nodes);
    cfg.service(post_node_lidt);
    cfg.service(post_node_alignment_isometry);
    cfg.service(post_node_property);
    cfg.service(post_node_isometry);
    cfg.service(post_node_inversion);
    cfg.service(post_analyzer_config);

    cfg.service(get_properties_ron);
    cfg.service(get_properties_json);
    cfg.service(get_analyzer_info_ron);
    cfg.service(get_analyzer_info_json);
    cfg.service(get_node_hierarchy);
    cfg.service(patch_properties);

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
