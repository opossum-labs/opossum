use std::collections::{HashMap, HashSet};

use crate::{
    app_state::{AppState, NodeCacheItem},
    error::BackEndErrorResponse,
    helper_functions::{
        add_converted_group_to_scenery, build_new_group_from_refs_and_conns,
        collect_group_connections, collect_node_refs_and_pos, connect_from_info,
        create_new_group_node_info, split_sort_connections,
    },
};
use actix_web::{
    post,
    web::{self, Json},
};
use nalgebra::Point2;
use opossum_core::{
    core_optics::OpticRef,
    nodes::{ConnectionInfo, NodeGroup, create_node_ref},
    opm_document::AnalyzerInfo,
    prelude::{OpticNode, PortMap, PortType, Proptype},
    types::api_types::{
        ConnectInfo, ConvertToGroupRequest, ErrorResponse, MoveNodesRequest, NodeInfo,
    },
    utils::LockExt,
};
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

/// Copy existing nodes
///
/// This function copies a single or multiple already existing nodes
#[utoipa::path(tag = "operations",
    request_body(content = HashSet<Uuid>,
        description = "List of Uuids of the nodes to be copied",
        content_type = "application/json",
    ),
    responses(
        (status = OK, body= NodeInfo, description = "Node successfully created", content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/copy_nodes")]
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
/// Delete all nodes that have been cut out previously and
///
/// This function sends already copied nodes to the frontend
#[utoipa::path(tag = "operations",
    responses(
        (status = OK, body= NodeInfo, description = "Cut-out nodes successfully  removed and cache cleared", content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[allow(clippy::significant_drop_tightening)]
#[post("/cut_nodes")]
async fn post_cut_nodes(
    data: web::Data<AppState>,
    paste_in_group_id: web::Json<Uuid>,
) -> Result<Json<(Vec<Uuid>, Uuid)>, BackEndErrorResponse> {
    let paste_in_group_id = paste_in_group_id.into_inner();
    let mut nodes_to_delete = vec![];
    let mut analyzers_to_delete = vec![];
    let mut node_cache = data.node_copy_cache.lock();
    while let Some(cache) = node_cache.pop() {
        match cache {
            NodeCacheItem::Optical(optic_ref) => {
                nodes_to_delete.push(optic_ref.uuid());
            }
            NodeCacheItem::Analyzer(analyzer_info) => {
                analyzers_to_delete.push(analyzer_info.id());
            }
        }
    }
    drop(node_cache);

    let mut document = data.document.lock();
    let mut deleted_nodes = vec![];
    let scenery = document.scenery();
    let scenery_id = scenery.node_attr().uuid();

    let group_id = if analyzers_to_delete.is_empty()
        && let Some(id) = nodes_to_delete.first()
    {
        let (_, group_id) = scenery.node_recursive(*id)?;
        group_id
    } else {
        scenery_id
    };

    if scenery_id == paste_in_group_id {
        for analyzer in &analyzers_to_delete {
            deleted_nodes.push(*analyzer);
            document.remove_analyzer(*analyzer)?;
        }
    }

    let scenery = document.scenery_mut();

    for node in &nodes_to_delete {
        deleted_nodes.extend(scenery.delete_node(*node)?);
    }

    Ok(Json((deleted_nodes, group_id)))
}
/// Paste copied nodes
///
/// This function sends already copied nodes to the frontend
#[utoipa::path(tag = "operations",
    request_body(content = (Uuid, (f64, f64)),
        description = "Uuid of the group node to be pasted in, the position at which the node should be pasted",
        content_type = "application/json",
    ),
    responses(
        (status = OK, body= NodeInfo, description = "Node successfully pasted", content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[allow(clippy::significant_drop_tightening)]
#[post("/paste_nodes")]
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
    let (paste_group_id, node_pos) = node_paste_info.into_inner();
    let paste_in_scenery = data.document.lock().scenery().node_attr().uuid() == paste_group_id;

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
    let mut grouped_node_refs = Vec::<(Uuid, Vec<OpticRef>, bool)>::new();
    let mut grouped_node_infos = HashMap::<Uuid, Vec<NodeInfo>>::new();
    let mut grouped_connect_info = HashMap::<Uuid, Vec<ConnectInfo>>::new();
    let mut grouped_connections =
        HashMap::<Uuid, (HashMap<Uuid, Vec<ConnectionInfo>>, bool)>::new();
    let mut node_id_link = HashMap::<Uuid, Uuid>::new();
    let mut input_port_maps = HashMap::<Uuid, PortMap>::new();
    let mut output_port_maps = HashMap::<Uuid, PortMap>::new();
    // node_id_link.insert(paste_group_id, paste_group_id);
    collect_optical_nodes_to_copy_recursive(
        scenery,
        paste_group_id,
        shift,
        &copied_optical_nodes,
        &mut node_id_link,
        &mut grouped_connections,
        &mut grouped_node_refs,
        &mut input_port_maps,
        &mut output_port_maps,
        true,
    )?;

    for (group_id, node_refs, is_root_group) in grouped_node_refs.iter().rev() {
        // if let Some(mapped_group_id) = node_id_link.get(group_id){

        let mapped_group_id_opt = if *is_root_group {
            Some(*group_id)
        } else {
            node_id_link.get(group_id).copied()
        };
        if let Some(mapped_group_id) = mapped_group_id_opt {
            let mut node_info = Vec::new();
            for node_ref in node_refs {
                scenery
                    .with_group_node_mut(mapped_group_id, |g| g.add_node_ref(node_ref.clone()))??;
                let node = node_ref.optical_ref.lock_opm()?;
                node_info.push(NodeInfo::from_analyzable(&*node, None, None));
                drop(node);
            }
            grouped_node_infos.insert(mapped_group_id, node_info);
        }
    }

    resolve_references(scenery, &node_id_link)?;

    reconfigure_ports(
        scenery,
        &input_port_maps,
        &output_port_maps,
        &node_id_link,
        &mut grouped_node_infos,
    )?;

    for (group_id, (connections, is_root_group)) in &mut grouped_connections {
        let mapped_group_id_opt = if *is_root_group {
            Some(*group_id)
        } else {
            node_id_link.get(group_id).copied()
        };
        if let Some(mapped_group_id) = mapped_group_id_opt {
            remap_connections(connections, &node_id_link);
            let connect_info = set_copied_connections(scenery, mapped_group_id, connections)?;
            grouped_connect_info.insert(mapped_group_id, connect_info);
        }
    }
    Ok(Json((grouped_node_infos, analyzers, grouped_connect_info)))
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
#[allow(clippy::too_many_arguments)]
pub fn collect_optical_nodes_to_copy_recursive(
    scenery: &mut NodeGroup,
    group_id_to_insert: Uuid,
    shift: Point2<f64>,
    copied_optical_nodes: &[OpticRef],
    node_id_link: &mut HashMap<Uuid, Uuid>,
    grouped_connect_info: &mut HashMap<Uuid, (HashMap<Uuid, Vec<ConnectionInfo>>, bool)>,
    grouped_node_infos: &mut Vec<(Uuid, Vec<OpticRef>, bool)>,
    input_port_maps: &mut HashMap<Uuid, PortMap>,
    output_port_maps: &mut HashMap<Uuid, PortMap>,
    is_root_group: bool,
) -> Result<(), BackEndErrorResponse> {
    let mut optical_nodes = Vec::new();
    grouped_connect_info.insert(
        group_id_to_insert,
        (HashMap::<Uuid, Vec<ConnectionInfo>>::new(), is_root_group),
    );
    for node in copied_optical_nodes {
        let node_id = node.uuid();

        let group_nodes_opt = {
            let guard = node.optical_ref.lock_opm()?;

            guard.as_group().map_or_else(
                |_| None,
                |group| {
                    input_port_maps
                        .insert(node_id, group.graph().port_map(&PortType::Input).clone());
                    output_port_maps
                        .insert(node_id, group.graph().port_map(&PortType::Output).clone());

                    Some(group.nodes().iter().copied().cloned().collect::<Vec<_>>())
                },
            )
        };
        let copied_node = collect_optical_node_to_copy(
            scenery,
            group_id_to_insert,
            shift,
            node,
            node_id_link,
            grouped_connect_info,
        )?;

        optical_nodes.push(copied_node);

        if let Some(nodes_in_group) = group_nodes_opt {
            collect_optical_nodes_to_copy_recursive(
                scenery,
                node_id,
                Point2::origin(),
                &nodes_in_group,
                node_id_link,
                grouped_connect_info,
                grouped_node_infos,
                input_port_maps,
                output_port_maps,
                false,
            )?;
        }
    }
    grouped_node_infos.push((group_id_to_insert, optical_nodes, is_root_group));
    Ok(())
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
pub fn collect_optical_node_to_copy(
    scenery: &NodeGroup,
    group_id: Uuid,
    shift: Point2<f64>,
    optic_ref: &OpticRef,
    node_id_link: &mut HashMap<Uuid, Uuid>,
    grouped_connect_info: &mut HashMap<Uuid, (HashMap<Uuid, Vec<ConnectionInfo>>, bool)>,
) -> Result<OpticRef, BackEndErrorResponse> {
    let (new_node_ref, old_node_id) = copy_from_optic_ref(scenery, optic_ref)?;

    let new_pos = get_shifted_pos_of_ref(optic_ref, shift)?;

    let mut node = new_node_ref.optical_ref.lock_opm()?;
    let node_attr = node.node_attr_mut();
    node_attr.set_gui_position(Some(Point2::new(new_pos.0, new_pos.1)));

    drop(node);

    node_id_link.insert(old_node_id, new_node_ref.uuid());

    let parent_group_id = scenery.node_recursive(old_node_id)?.1;

    let connect = scenery.with_group_node(parent_group_id, |group| {
        group
            .graph()
            .get_outgoing_connection_info_of_node(old_node_id)
    })?;

    if let Some((c_info_map, _)) = grouped_connect_info.get_mut(&group_id) {
        c_info_map.insert(old_node_id, connect);
    }

    Ok(new_node_ref)
}
pub fn copy_from_optic_ref(
    scenery: &NodeGroup,
    optic_ref: &OpticRef,
) -> Result<(OpticRef, Uuid), BackEndErrorResponse> {
    let (old_node_id, reference_uuid_opt, node_type, node_attr_clone) = {
        let node = optic_ref.optical_ref.lock_opm()?;

        let old_node_id = node.node_attr().uuid();
        let reference_uuid_opt = node
            .node_attr()
            .properties()
            .get("reference id")
            .ok()
            .and_then(|p| match p {
                Proptype::Uuid(id) => Some(*id),
                _ => None,
            });

        let node_type = node.node_type();
        let node_attr_clone = node.node_attr().clone();
        drop(node);

        (old_node_id, reference_uuid_opt, node_type, node_attr_clone)
    };

    let referenced_node_opt = if let Some(ref_uuid) = reference_uuid_opt {
        Some(scenery.node_recursive(ref_uuid)?.0)
    } else {
        None
    };

    let new_node_ref = create_node_ref(&node_type)?;
    let mut node = new_node_ref.optical_ref.lock_opm()?;
    if let Some(referenced_node) = referenced_node_opt {
        node.as_refnode_mut()?.assign_reference(&referenced_node);
    }

    let node_attr = node.node_attr_mut();
    node_attr.replace_from_node_attr(&node_attr_clone);

    drop(node);

    Ok((new_node_ref, old_node_id))
}
fn get_shifted_pos_of_ref(
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

/// Convert the given nodes into a new subgroup within an existing group.
///
/// The request body must contain the ID of the source group (`group_id`) and a
/// list of node UUIDs (`nodes_to_convert`) that will be removed from the source
/// group and wrapped into a newly created group node.
#[utoipa::path(
    tag = "operations",
    // params(...) wurde komplett entfernt!
    request_body(
        content = ConvertToGroupRequest,
        description = "Information about the parent group and the nodes to convert",
        content_type = "application/json"
    ),
    responses(
        (status = OK, description = "Nodes successfully converted to group"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/convert_to_group")]
pub async fn post_convert_nodes_to_group(
    data: web::Data<AppState>,
    request: web::Json<ConvertToGroupRequest>, // <-- Wir nehmen nun unser neues Struct entgegen
) -> Result<Json<(NodeInfo, Vec<ConnectInfo>)>, BackEndErrorResponse> {
    // Entpacke die Daten aus dem Request-Body
    let req = request.into_inner();
    let group_id = req.group_id;
    let nodes_to_convert = req.nodes_to_convert;

    //collect data
    let (node_refs, pos) = collect_node_refs_and_pos(&data, &nodes_to_convert);
    let all_connections = collect_group_connections(&data, group_id)?;
    let split = split_sort_connections(&data, &all_connections, &nodes_to_convert);

    //create new group: add nodes and connections
    let new_group = build_new_group_from_refs_and_conns(node_refs, &split)?;

    //addnew group to scenery
    let new_group_id = add_converted_group_to_scenery(
        &data,
        group_id,
        nodes_to_convert,
        new_group,
        &split.input,
        &split.output,
    )?;

    //create the nodeinfo struct for the GUI
    let new_group_node_info = create_new_group_node_info(&data, new_group_id, pos)?;

    let mut external_connections = split.input;
    external_connections.extend(split.output);

    Ok(Json((new_group_node_info, external_connections)))
}

/// Move the given nodes from one group into another group.
///
/// All specified nodes will be removed from the source group and inserted into
/// the target group, including their internal connections.
#[utoipa::path(
    tag = "operations",
    // params(...) wurde komplett entfernt!
    request_body(
        content = MoveNodesRequest,
        description = "Information about the source group, target group, and nodes to move",
        content_type = "application/json"
    ),
    responses(
        (status = OK, description = "Nodes successfully transferred to group"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/move_nodes")]
pub async fn post_move_nodes(
    data: web::Data<AppState>,
    request: web::Json<MoveNodesRequest>, // <-- Wir nehmen das neue Struct entgegen
) -> Result<(), BackEndErrorResponse> {
    // Entpacke die Daten aus dem Request-Body
    let req = request.into_inner();
    let from_group_id = req.source_group_id;
    let drop_group_id = req.target_group_id;
    let mut nodes_to_drop = req.nodes_to_move;

    //collect data
    let (node_refs, _) = collect_node_refs_and_pos(&data, &nodes_to_drop);
    let all_connections = collect_group_connections(&data, from_group_id)?;
    let split = split_sort_connections(&data, &all_connections, &nodes_to_drop);

    let mut document = data.document.lock();
    let scenery: &mut opossum_core::prelude::NodeGroup = document.scenery_mut();

    //delete nodes_to_drop from original scenery. Important to do this befor inserting the optic_refs as they will then also removed
    while let Some(node) = nodes_to_drop.pop() {
        let deleted = scenery.delete_node(node)?;
        for del_id in &deleted {
            nodes_to_drop.retain(|id| id != del_id);
        }
    }

    //add nodes_to_drop to group
    for node_ref in &node_refs {
        scenery.with_group_node_mut(drop_group_id, |g| g.add_node_ref(node_ref.clone()))??;
    }

    //connect nodes if there are any
    for conn in &split.inside {
        scenery.with_group_node_mut(drop_group_id, |g| connect_from_info(g, conn))??;
    }

    drop(document);
    Ok(())
}

pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(post_copy_nodes);
    cfg.service(post_cut_nodes);
    cfg.service(post_paste_nodes);

    cfg.service(post_convert_nodes_to_group);
    cfg.service(post_move_nodes);
}
