use std::collections::{HashMap, HashSet};

use crate::{
    app_state::{AppState, NodeCacheItem}, error::BackEndErrorResponse, utils::update_node_attr
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
    error::{OpmResult, OpossumError},
    meter,
    nodes::{
        ConnectionInfo, NodeAttr, NodeGroup, NodeReference, create_node_ref,
        fluence_detector::Fluence,
    },
    opm_document::AnalyzerInfo,
    optic_ports::PortType,
    prelude::{OpmDocument, OpticNode},
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
#[get("/{uuid}/connections")]
pub async fn get_connections(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<ConnectInfo>>, BackEndErrorResponse> {
    let document = data.document.lock();
    let scenery = document.scenery().clone();
    drop(document);

    let uuid = path.into_inner();
    let connections = scenery.with_group_node(uuid, |g| {
        g.connections().clone() 
    })?;
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
                node.gui_position().unwrap()
            }
            NodeCacheItem::Analyzer(analyzer) => analyzer.gui_position().unwrap(),
        };

        corner.x = corner.x.min(pos.x);
        corner.y = corner.y.min(pos.y);
    }

    Ok(corner)
}

fn copy_optical_node(
    data: &web::Data<AppState>,
    group_id: Uuid,
    node_pos: (f64, f64),
    min_pos: Point2<f64>,
    optic_ref: &OpticRef,
    node_id_link: &mut HashMap<Uuid, Uuid>,
    connections: &mut HashMap<Uuid, Vec<ConnectionInfo>>,
) -> Result<NodeInfo, BackEndErrorResponse> {
    let node_to_copy_from = optic_ref.optical_ref.lock_opm()?;
    let old_node_id = node_to_copy_from.node_attr().uuid();

    let new_node_ref = create_node_ref(&node_to_copy_from.node_type())?;
    let mut node = new_node_ref.optical_ref.lock_opm()?;

    let mut document = data.document.lock();
    if let Ok(Proptype::Uuid(ref_uuid)) = node_to_copy_from
        .node_attr()
        .properties()
        .get("reference id")
        && let Ok(ref_node) = node.as_refnode_mut()
    {
        let (referenced_node, _) = document.scenery().node_recursive(*ref_uuid)?;
        ref_node.assign_reference(&referenced_node);
    }

    let node_attr = node.node_attr_mut();
    node_attr.replace_from_node_attr(node_to_copy_from.node_attr());

    let old_pos = node_to_copy_from.gui_position().unwrap();
    let new_pos = (
        node_pos.0 + (old_pos.x - min_pos.x),
        node_pos.1 + (old_pos.y - min_pos.y),
    );

    node_attr.set_gui_position(Some(Point2::new(new_pos.0, new_pos.1)));

    drop(node_to_copy_from);
    drop(node);

    let scenery = document.scenery_mut();

    let new_node_uuid =
        scenery.with_group_node_mut(group_id, |g| g.add_node_ref(new_node_ref.clone()))??;

    node_id_link.insert(old_node_id, new_node_uuid);

    scenery.with_group_node(group_id, |group| {
        let connect = group
            .graph()
            .clone()
            .get_outgoing_connection_info_of_node(old_node_id);
        connections.insert(old_node_id, connect);
    })?;

    drop(document);

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

fn copy_analyzer(
    data: &web::Data<AppState>,
    node_pos: (f64, f64),
    min_pos: Point2<f64>,
    analyzer: &AnalyzerInfo,
) -> AnalyzerInfo {
    let old_pos = analyzer.gui_position().unwrap();

    let new_pos = Point2::new(
        node_pos.0 + (old_pos.x - min_pos.x),
        node_pos.1 + (old_pos.y - min_pos.y),
    );

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
    connections: HashMap<Uuid, Vec<ConnectionInfo>>,
) -> Result<Vec<ConnectInfo>, BackEndErrorResponse> {
    let mut result = Vec::new();

    for (_, conns) in connections {
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
    data: &web::Data<AppState>,
    node_id_link: &HashMap<Uuid, Uuid>,
) -> Result<(), BackEndErrorResponse> {
    let mut document = data.document.lock();
    let scenery = document.scenery_mut();

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
) -> Result<Json<(Vec<NodeInfo>, Vec<AnalyzerInfo>, Vec<ConnectInfo>)>, BackEndErrorResponse> {
    let (group_id, node_pos) = node_paste_info.into_inner();

    let copied_nodes = data.node_copy_cache.lock();

    let min_pos = upper_left_corner_of_nodes(&copied_nodes)?;

    let mut node_id_link = HashMap::new();
    let mut connections = HashMap::new();
    let mut optical_nodes = Vec::new();
    let mut analyzers = Vec::new();

    for node in copied_nodes.iter() {
        match node {
            NodeCacheItem::Optical(optical) => {
                optical_nodes.push(copy_optical_node(
                    &data,
                    group_id,
                    node_pos,
                    min_pos,
                    optical,
                    &mut node_id_link,
                    &mut connections,
                )?);
            }
            NodeCacheItem::Analyzer(analyzer) => {
                analyzers.push(copy_analyzer(&data, node_pos, min_pos, analyzer));
            }
        }
    }
    drop(copied_nodes);
    resolve_references(&data, &node_id_link)?;

    remap_connections(&mut connections, &node_id_link);

    let mut document = data.document.lock();
    let scenery = document.scenery_mut();

    let connect_info = set_copied_connections(scenery, group_id, connections)?;

    Ok(Json((optical_nodes, analyzers, connect_info)))
}

/// Convert a set of nodes to a group node by creating a new group node and instering the nodes
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of the group in which the nodes are currently contained"),
    ),
    request_body(content = String,
        description = "Set of node uuids that correspond to the nodes that should be converted to a group",
        content_type = "application/json",
    ),
    responses(
        (status = OK, description = "Nodes successfully converted to group"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/{uuid}/convertToGroup")]
pub async fn post_convert_nodes_to_group(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    nodes_to_convert: web::Json<Vec<Uuid>>,
) -> Result<Json<(NodeInfo, Vec<ConnectInfo>)>, BackEndErrorResponse> {
    let group_id = path.into_inner();
    let nodes_to_convert = nodes_to_convert.into_inner();

    let (node_refs, pos) = collect_node_refs_and_pos(&data, &nodes_to_convert);

    let all_connections = collect_group_connections(&data, group_id)?;
    let reference_map = build_reference_map(&data, &all_connections);
    let (inside_connections, map_input_connections, map_output_connections) =
        split_connections(&all_connections, &reference_map, &nodes_to_convert);

    let new_group = build_new_group(
        node_refs,
        &inside_connections,
        &map_input_connections,
        &map_output_connections,
    )?;

    let new_group_id = add_converted_group_to_scenery(
        &data,
        group_id,
        &nodes_to_convert,
        new_group,
        &map_input_connections,
        &map_output_connections,
    )?;

    let new_group_node_info = create_new_group_node_info(&data, new_group_id, pos)?;

    let mut all_external_connections = map_input_connections;
    all_external_connections.extend(map_output_connections);

    Ok(Json((new_group_node_info, all_external_connections)))
}
fn collect_node_refs_and_pos(
    data: &web::Data<AppState>,
    nodes_to_convert: &[Uuid],
) -> (Vec<OpticRef>, Point2<f64>) {
    let document = data.document.lock();
    let scenery = document.scenery();
    let mut corner = Point2::new(f64::INFINITY, f64::INFINITY);
    let optic_ref_vec = nodes_to_convert
        .iter()
        .filter_map(|node| scenery.node_recursive(*node).ok().map(|(r, _)| {
            if let Ok(opt_ref) = r.optical_ref.lock_opm(){
                let pos = opt_ref.gui_position().unwrap();
                corner.x = corner.x.min(pos.x);
                corner.y = corner.y.min(pos.y);
            }
            r
        }))
        .collect();
    (optic_ref_vec, corner)
}

fn collect_group_connections(
    data: &web::Data<AppState>,
    group_id: Uuid,
) -> OpmResult<Vec<ConnectionInfo>> {
    let document = data.document.lock();
    let scenery = document.scenery();

    scenery.with_group_node(group_id, |group| group.connections().clone())
}

fn build_reference_map(
    data: &web::Data<AppState>,
    connections: &[ConnectionInfo],
) -> std::collections::HashMap<Uuid, bool> {
    let document = data.document.lock();
    let scenery = document.scenery();

    connections
        .iter()
        .map(|c| {
            let is_ref = scenery
                .with_node_attr(c.target_id, |attr| {
                    attr.properties().get("reference id").is_ok()
                })
                .unwrap_or(false);
            (c.target_id, is_ref)
        })
        .collect()
}

fn split_connections(
    connections: &[ConnectionInfo],
    reference_map: &std::collections::HashMap<Uuid, bool>,
    nodes_to_convert: &[Uuid],
) -> (
    Vec<ConnectInfo>,
    Vec<ConnectInfo>,
    Vec<ConnectInfo>,
) {
    let mut inside = Vec::new();
    let mut input = Vec::new();
    let mut output = Vec::new();

    for c in connections {
        let is_reference = *reference_map.get(&c.target_id).unwrap_or(&false);
        let c_info = ConnectInfo::from_connection_info(c, is_reference);

        let src_inside = nodes_to_convert.contains(&c_info.src_uuid());
        let tgt_inside = nodes_to_convert.contains(&c_info.target_uuid());

        match (src_inside, tgt_inside) {
            (true, true) => inside.push(c_info),
            (true, false) => output.push(c_info),
            (false, true) => input.push(c_info),
            _ => {}
        }
    }

    (inside, input, output)
}

fn build_new_group(
    node_refs: Vec<OpticRef>,
    inside_connections: &[ConnectInfo],
    map_input_connections: &[ConnectInfo],
    map_output_connections: &[ConnectInfo],
) -> OpmResult<NodeGroup> {
    let mut new_group = NodeGroup::new("new group");

    for node_ref in node_refs {
        new_group.add_node_ref(node_ref)?;
    }

    for conn in inside_connections {
        new_group.connect_nodes(
            conn.src_uuid(),
            conn.src_port(),
            conn.target_uuid(),
            conn.target_port(),
            meter!(conn.distance()),
        )?;
    }

    for map_out in map_output_connections {
        new_group.map_output_port(map_out.src_uuid(), map_out.src_port(), map_out.src_port())?;
    }

    for map_in in map_input_connections {
        new_group.map_input_port(map_in.target_uuid(), map_in.target_port(), map_in.target_port())?;
    }

    Ok(new_group)
}

fn add_converted_group_to_scenery(
    data: &web::Data<AppState>,
    group_id: Uuid,
    nodes_to_convert: &[Uuid],
    new_group: NodeGroup,
    map_input_connections: &[ConnectInfo],
    map_output_connections: &[ConnectInfo],
) -> Result<Uuid, BackEndErrorResponse> {
    let mut document = data.document.lock();
    let scenery = document.scenery_mut();

    for node in nodes_to_convert {
        scenery.delete_node(*node)?;
    }

    scenery.with_group_node_mut(group_id, |g| {match g.add_node(new_group){
Ok(new_group_id) => {
    //connect the output ports and connect within scenery
    for  map_out in map_output_connections{
        g.connect_nodes(map_out.src_uuid(), map_out.src_port(), map_out.target_uuid(), map_out.target_port(), meter!(map_out.distance()))?;
    }
    //connect the input ports
    for  map_in in map_input_connections{
        g.connect_nodes(map_in.src_uuid(), map_in.src_port(), map_in.target_uuid(), map_in.target_port(), meter!(map_in.distance()))?;
    }
    Ok(new_group_id)
}
Err(e) =>             Err(BackEndErrorResponse::new(404, "Opossum", &format!("Could not add group node{e}")))}})?

}

fn create_new_group_node_info(
    data: &web::Data<AppState>,
    new_group_id: Uuid,
    pos: Point2<f64>
) -> OpmResult<NodeInfo> {
    let document = data.document.lock();
    let scenery = document.scenery();

    let (new_group_ref, _) = scenery.node_recursive(new_group_id)?;
    let new_group_node = new_group_ref.optical_ref.lock_opm()?;



    Ok(NodeInfo::new(
        new_group_id,
        new_group_node.name(),
        new_group_node.inverted(),
        new_group_node.node_type(),
        new_group_node.ports().names(&PortType::Input),
        new_group_node.ports().names(&PortType::Output),
        Some((pos.x, pos.y)),
    ))
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
    let nodes_to_rename = scenery.graph().find_all_nodes_referring_to_uuid(uuid);
    for node_idx in &nodes_to_rename {
        let node_uuid = scenery.graph().node_by_idx(*node_idx).unwrap().uuid();
        scenery
            .with_node_attr_mut(node_uuid, |node_attr| {
                let name = if node_attr.node_type() == "reference" {
                    format!("ref ({name})")
                } else {
                    name.clone()
                };
                node_attr.set_name(&name);
                processed_names.insert(node_uuid, name);
            })
            .map_err(|_| BackEndErrorResponse::new(404, "Opossum", "uuid not found in nodes"))?;
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
    cfg.service(post_convert_nodes_to_group);
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
