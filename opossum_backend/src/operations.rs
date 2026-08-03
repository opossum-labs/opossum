use std::collections::{HashMap, HashSet};

use crate::{
    app_state::{AppState, NodeCacheItem},
    document::apply_position_updates,
    error::BackEndErrorResponse,
    helper_functions::{
        capture_node_connections, collect_group_connections, collect_node_refs_and_pos,
        create_new_group_node_info, is_reference_target, lowest_common_ancestor_group,
        parent_group_id_or_self, relocate_nodes_in_document, split_sort_connections,
    },
    undo::{Command, EdgeSnapshot, GroupConversion, MoveNodes, NodeSnapshot, ReroutedMapping},
};
use actix_web::{
    post,
    web::{self, Json},
};
use nalgebra::Point2;
use opossum_core::{
    core_optics::{NodeAttrExt, OpticRef, node_attr::HasNodeAttr},
    error::{OpmResult, OpossumError},
    nodes::{ConnectionInfo, NodeGroup, NodeReference, create_node_ref},
    opm_document::AnalyzerInfo,
    prelude::{OpticNode, PortMap, PortType, Proptype},
    types::api_types::{
        AnalyzerItemDto, ConnectInfo, ConvertToGroupRequest, ConvertToGroupResponse,
        CutNodesResponse, ErrorResponse, MoveNodesRequest, MoveNodesResponse, NodeInfo,
        PasteNodesResponse, PositionUpdate, RelocatedNode,
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

    // Get optic ref of node that should be copied
    let document = data.document.lock();
    let mut copied_nodes_set = data.node_copy_cache.lock();
    copied_nodes_set.clear();

    for id in &node_ids_to_copy {
        if let Ok((node_ref_to_copy, _)) = document.scenery().node_recursive(*id) {
            copied_nodes_set.push(NodeCacheItem::Optical(node_ref_to_copy));
        } else if let Some(analyzer) = document.analyzers().get(id).cloned() {
            // Save the DTO in cache so we retain the ID
            copied_nodes_set.push(NodeCacheItem::Analyzer(AnalyzerItemDto {
                id: *id,
                info: analyzer,
            }));
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

/// The pasted-in node/connection info [`insert_copied_nodes`] hands back to [`post_paste_nodes`].
struct PastedNodes {
    grouped_node_infos: HashMap<Uuid, Vec<NodeInfo>>,
    grouped_connect_info: HashMap<Uuid, Vec<ConnectInfo>>,
}

/// Copies `copied_optical_nodes` into `paste_group_id` (recursively, preserving group structure),
/// replays their port maps and reference targets, and reconnects their captured internal connections
/// through the fresh uuids - the whole "materialize a paste" phase of [`post_paste_nodes`], shared
/// between a plain paste and the paste half of a cut+paste.
///
/// # Errors
///
/// Returns an error if the recursive copy, reference resolution, port-map replay, or connection replay
/// step fails.
fn insert_copied_nodes(
    scenery: &mut NodeGroup,
    paste_group_id: Uuid,
    shift: Point2<f64>,
    copied_optical_nodes: &[OpticRef],
) -> Result<PastedNodes, BackEndErrorResponse> {
    let mut grouped_node_refs = Vec::<(Uuid, Vec<OpticRef>, bool)>::new();
    let mut grouped_node_infos = HashMap::<Uuid, Vec<NodeInfo>>::new();
    let mut grouped_connect_info = HashMap::<Uuid, Vec<ConnectInfo>>::new();
    let mut grouped_connections =
        HashMap::<Uuid, (HashMap<Uuid, Vec<ConnectionInfo>>, bool)>::new();
    let mut node_id_link = HashMap::<Uuid, Uuid>::new();
    let mut input_port_maps = HashMap::<Uuid, PortMap>::new();
    let mut output_port_maps = HashMap::<Uuid, PortMap>::new();

    collect_optical_nodes_to_copy_recursive(
        scenery,
        paste_group_id,
        shift,
        copied_optical_nodes,
        &mut node_id_link,
        &mut grouped_connections,
        &mut grouped_node_refs,
        &mut input_port_maps,
        &mut output_port_maps,
        true,
    )?;

    for (group_id, node_refs, is_root_group) in grouped_node_refs.iter().rev() {
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
                node_info.push(NodeInfo::from_analyzable(&*node, None));
                drop(node);
            }
            grouped_node_infos.insert(mapped_group_id, node_info);
        }
    }

    resolve_references(scenery, &node_id_link)?;

    reconfigure_ports(
        scenery,
        &grouped_node_refs,
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

    Ok(PastedNodes {
        grouped_node_infos,
        grouped_connect_info,
    })
}

/// Paste copied nodes
///
/// This function duplicates the nodes/analyzers currently in the copy cache into the target group,
/// minting a fresh uuid for each copy. Moving nodes without duplicating them (a "cut") is a separate
/// operation - see [`post_cut_nodes`].
#[utoipa::path(tag = "operations",
    request_body(content = (Uuid, (f64, f64)),
        description = "Uuid of the group node to be pasted in, and the position at which the node should be pasted",
        content_type = "application/json",
    ),
    responses(
        (status = OK, body= PasteNodesResponse, description = "Node successfully pasted", content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[allow(clippy::significant_drop_tightening)]
#[post("/paste_nodes")]
async fn post_paste_nodes(
    data: web::Data<AppState>,
    node_paste_info: web::Json<(Uuid, (f64, f64))>,
) -> Result<Json<PasteNodesResponse>, BackEndErrorResponse> {
    let (paste_group_id, node_pos) = node_paste_info.into_inner();
    let paste_in_scenery = data.document.lock().scenery().node_attr().uuid() == paste_group_id;

    let copied_nodes = data.node_copy_cache.lock();
    let min_pos = upper_left_corner_of_nodes(&copied_nodes)?;
    drop(copied_nodes);
    let shift = Point2::new(node_pos.0 - min_pos.x, node_pos.1 - min_pos.y);

    let mut copied_optical_nodes = Vec::<OpticRef>::new();
    let mut copied_analyzer_nodes = Vec::<AnalyzerItemDto>::new();

    for cache in data.node_copy_cache.lock().iter() {
        match cache {
            NodeCacheItem::Optical(optic_ref) => copied_optical_nodes.push(optic_ref.clone()),
            NodeCacheItem::Analyzer(analyzer_dto) => {
                copied_analyzer_nodes.push(analyzer_dto.clone());
            }
        }
    }

    let mut analyzers = Vec::new();
    if paste_in_scenery {
        for analyzer_dto in &copied_analyzer_nodes {
            // Pass the internal AnalyzerInfo to copy_analyzer
            analyzers.push(copy_analyzer(&data, shift, &analyzer_dto.info));
        }
    }

    let mut document = data.document.lock();
    let PastedNodes {
        grouped_node_infos,
        grouped_connect_info,
    } = insert_copied_nodes(
        document.scenery_mut(),
        paste_group_id,
        shift,
        &copied_optical_nodes,
    )?;

    // One paste = one undo step: removing every pasted node/analyzer undoes the whole paste at once.
    let mut removals = Vec::new();
    // Only the *top-level* pasted roots need their own `RemoveNode` - a group's own `OpticRef`
    // already carries its entire internal subtree (nodes and internal edges) as one live object,
    // so removing it via a single `RemoveNode` already correctly captures/restores everything
    // inside it. Giving a nested descendant (an entry under any *other* key of
    // `grouped_node_infos` - a freshly-created nested group's own uuid) its own separate
    // `RemoveNode` too is not just redundant but harmful: `Command::Batch` applies its commands in
    // `Vec` order, itself derived from this map's non-deterministic iteration order, so a nested
    // entry can end up targeting a uuid its own ancestor's `RemoveNode` already cascaded away
    // (surfacing as "node with given uuid does not exist" on undo), or mutate the group's live
    // internal graph directly *before* the group's own command runs, silently severing an internal
    // connection that nothing later restores (a nested `AddNode` always passes an empty
    // `connections` list, so redo can't reconnect what this already tore down).
    if let Some(infos) = grouped_node_infos.get(&paste_group_id) {
        // A freshly pasted node's only connections are to other nodes pasted in the same gesture
        // (`insert_copied_nodes` only ever recreates connections between copied nodes) - so every
        // connection touching a top-level pasted node is "mutual" in `capture_and_split_mutual_
        // connections`'s sense. Restore each one once via a *leading* `RemoveEdge`, positioned before
        // the `RemoveNode`s below: on the first undo this disconnects the pair while both nodes still
        // exist, and thanks to `Command::Batch` reversing its inverses, the resulting redo batch adds
        // both nodes back before restoring the edge - never the other way around, which would try to
        // reconnect to a node redo hasn't re-added yet.
        let top_level_ids: HashSet<Uuid> = infos.iter().map(NodeInfo::uuid).collect();
        let mut seen_mutual = HashSet::new();
        for info in infos {
            let (_own, mutual) = capture_and_split_mutual_connections(
                document.scenery(),
                paste_group_id,
                info.uuid(),
                &top_level_ids,
                &mut seen_mutual,
            );
            for c in mutual {
                removals.push(Command::RemoveEdge(EdgeSnapshot {
                    group_id: paste_group_id,
                    connect_info: c,
                }));
            }
        }
        for info in infos {
            if let Ok((node_ref, _)) = document.scenery().node_recursive(info.uuid()) {
                removals.push(Command::RemoveNode(NodeSnapshot {
                    parent_group_id: paste_group_id,
                    node: node_ref,
                    cascaded: Vec::new(),
                    connections: Vec::new(),
                }));
            }
        }
    }
    for analyzer in &analyzers {
        removals.push(Command::RemoveAnalyzer(analyzer.clone()));
    }

    if !removals.is_empty() {
        data.push_undo(Command::Batch(removals));
    }

    Ok(Json(PasteNodesResponse {
        pasted_nodes: grouped_node_infos,
        pasted_analyzers: analyzers,
        pasted_connections: grouped_connect_info,
    }))
}

/// Cut the copy cache into a group as a UUID-preserving *move*.
///
/// A cut relocates the *same* nodes rather than duplicating them (as [`post_paste_nodes`] does): each keeps
/// its uuid, so every reference, port map and connection pointing at it stays valid with no remapping, and
/// nothing is cascade-deleted. A cut node already in `target_group_id` is only repositioned (the common
/// "cut and paste in the same scenery" case); a node from another group is relocated into `target_group_id`
/// via the shared move machinery ([`relocate_nodes_in_document`]). Analyzers can only ever be repositioned
/// at the scenery root. The whole gesture is pushed as a single undo step (one [`Command::Batch`] of the
/// relocations' inverse [`Command::MoveNodes`] plus the reposition inverses), so undo/redo revert it in one
/// go - and because no uuid ever changes, redo can never hit the reference-cascade that a duplicate cut did.
///
/// # Errors
///
/// Returns an error if the copy cache position can't be read, `target_group_id` doesn't resolve, or a
/// relocation/reposition step fails (e.g. an attempt to move a group into its own descendant, which leaves
/// the destination unreachable once the group is detached from its source).
#[utoipa::path(tag = "operations",
    request_body(content = (Uuid, (f64, f64)),
        description = "Uuid of the group node to cut into, and the position at which the nodes should be placed",
        content_type = "application/json",
    ),
    responses(
        (status = OK, body = CutNodesResponse, description = "Nodes successfully cut (moved) into the group", content_type = "application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type = "application/json")
    )
)]
#[allow(clippy::significant_drop_tightening)]
#[post("/cut_nodes")]
async fn post_cut_nodes(
    data: web::Data<AppState>,
    body: web::Json<(Uuid, (f64, f64))>,
) -> Result<Json<CutNodesResponse>, BackEndErrorResponse> {
    let (target_group_id, pos) = body.into_inner();

    // Drain the copy cache (a cut is one-shot). Compute the paste shift from the cached nodes' current
    // top-left corner before draining them out.
    let mut cache = data.node_copy_cache.lock();
    let min_pos = upper_left_corner_of_nodes(&cache)?;
    let shift = Point2::new(pos.0 - min_pos.x, pos.1 - min_pos.y);
    let mut optical_ids = Vec::<Uuid>::new();
    let mut analyzer_ids = Vec::<Uuid>::new();
    for item in cache.drain(..) {
        match item {
            NodeCacheItem::Optical(optic_ref) => optical_ids.push(optic_ref.uuid()?),
            NodeCacheItem::Analyzer(dto) => analyzer_ids.push(dto.id),
        }
    }
    drop(cache);

    let mut document = data.document.lock();
    let scenery_root = document.scenery().node_attr().uuid();

    // Resolve each optical node's current parent group (skipping any that vanished since it was copied),
    // then split into "already in the target group" (reposition only) and "elsewhere" (relocate, keyed by
    // source group).
    let mut relocations = HashMap::<Uuid, Vec<Uuid>>::new();
    for id in &optical_ids {
        if let Ok((_, parent)) = document.scenery().node_recursive(*id)
            && parent != target_group_id
        {
            relocations.entry(parent).or_default().push(*id);
        }
    }

    // Relocate every out-of-group set into the target, preserving uuids. Aggregate the move side effects for
    // the GUI, and collect one inverse `MoveNodes` per source group for the undo batch. Sorted source order
    // keeps the pushed batch deterministic (`HashMap` iteration order isn't stable).
    let mut move_inverses = Vec::<Command>::new();
    let mut relocated_pairs = Vec::<(Uuid, Uuid)>::new();
    let mut new_connections = Vec::<(Uuid, ConnectInfo)>::new();
    let mut removed_connections = Vec::<(Uuid, ConnectInfo)>::new();
    let mut port_map_groups_changed = Vec::<Uuid>::new();
    let mut removed_port_mappings = Vec::<(Uuid, Uuid, String, PortType)>::new();
    let mut sources: Vec<Uuid> = relocations.keys().copied().collect();
    sources.sort();
    for source in sources {
        let Some(ids) = relocations.remove(&source) else {
            continue;
        };
        let outcome = relocate_nodes_in_document(&mut document, source, target_group_id, &ids)?;
        new_connections.extend(outcome.preserved.new_connections);
        removed_connections.extend(outcome.removed_connections);
        removed_port_mappings.extend(outcome.preserved.removed_port_mappings);

        let focus_group_id =
            lowest_common_ancestor_group(document.scenery(), source, target_group_id)?;
        let mut affected = outcome.preserved.port_map_groups_changed;
        affected.push(target_group_id);
        affected.sort();
        affected.dedup();
        port_map_groups_changed.extend(affected.iter().copied());
        move_inverses.push(Command::MoveNodes(MoveNodes {
            request: MoveNodesRequest {
                source_group_id: target_group_id,
                target_group_id: source,
                nodes_to_move: ids.clone(),
            },
            affected_groups: affected,
            focus_group_id,
        }));
        for id in ids {
            relocated_pairs.push((id, source));
        }
    }
    port_map_groups_changed.sort();
    port_map_groups_changed.dedup();
    let relocated_set: HashSet<Uuid> = relocated_pairs.iter().map(|(id, _)| *id).collect();

    // Reposition every cut node (relocated or same-group) - and every cut analyzer when pasting into the
    // root - to its shifted position. `PositionUpdate`s carry *absolute* positions, so add the shift to each
    // node's current position (a relocation preserves `gui_position`, so it's still the original).
    let mut position_updates = Vec::<PositionUpdate>::new();
    for id in &optical_ids {
        if let Ok(maybe_pos) = document.scenery().with_node_attr(*id, |a| a.gui_position()) {
            let (cx, cy) = maybe_pos.map_or((0.0, 0.0), |p| (p.x, p.y));
            position_updates.push(PositionUpdate {
                uuid: *id,
                is_optical: true,
                gui_position: (cx + shift.x, cy + shift.y),
            });
        }
    }
    if target_group_id == scenery_root {
        for id in &analyzer_ids {
            if let Ok(info) = document.analyzer(*id) {
                let (cx, cy) = info.gui_position().map_or((0.0, 0.0), |p| (p.x, p.y));
                position_updates.push(PositionUpdate {
                    uuid: *id,
                    is_optical: false,
                    gui_position: (cx + shift.x, cy + shift.y),
                });
            }
        }
    }
    // The GUI applies relocated nodes' new positions via `relocated_nodes` (baked into their `NodeInfo`),
    // so only the nodes/analyzers that stayed put need a standalone reposition entry in the response.
    let repositioned: Vec<PositionUpdate> = position_updates
        .iter()
        .filter(|u| !relocated_set.contains(&u.uuid))
        .cloned()
        .collect();
    let mut position_inverses = apply_position_updates(&mut document, position_updates)?;

    // Now that positions are final, capture each relocated node's `NodeInfo` for the target tab.
    let mut relocated_nodes = Vec::<RelocatedNode>::new();
    for (id, from_group_id) in relocated_pairs {
        if let Ok((node_ref, _)) = document.scenery().node_recursive(id) {
            let info = {
                let node = node_ref.optical_ref.lock_opm()?;
                NodeInfo::from_analyzable(&*node, None)
            };
            relocated_nodes.push(RelocatedNode {
                from_group_id,
                to_group_id: target_group_id,
                node: info,
            });
        }
    }

    // One undo step for the whole gesture: relocate-back moves, then reposition-back patches.
    let mut undo_batch = move_inverses;
    undo_batch.append(&mut position_inverses);
    if !undo_batch.is_empty() {
        data.push_undo(Command::Batch(undo_batch));
    }

    drop(document);
    Ok(Json(CutNodesResponse {
        relocated_nodes,
        repositioned,
        new_connections,
        removed_connections,
        port_map_groups_changed,
        removed_port_mappings,
    }))
}

/// Captures `node_id`'s connections in `parent_group_id`, splitting off any whose *other* endpoint is
/// also in `sibling_set` as "mutual" - deduped via `seen_mutual` (keyed by
/// `(src_uuid, src_port, target_uuid, target_port)`) so a connection reachable from both of its
/// endpoints is only reported once. Own (non-mutual) connections are meant for the node's own
/// `AddNode`/`RemoveNode` snapshot; mutual ones must instead go through a separate `AddEdge`/`RemoveEdge`
/// command placed outside the sibling nodes' own commands in the same batch - `Command::Batch` applies
/// (and reverses, see its `apply` impl) its commands in a fixed order, so a connection folded into one
/// sibling's own snapshot could be replayed before its other endpoint exists again.
fn capture_and_split_mutual_connections(
    scenery: &NodeGroup,
    parent_group_id: Uuid,
    node_id: Uuid,
    sibling_set: &HashSet<Uuid>,
    seen_mutual: &mut HashSet<(Uuid, String, Uuid, String)>,
) -> (Vec<ConnectInfo>, Vec<ConnectInfo>) {
    let all_connections =
        capture_node_connections(scenery, parent_group_id, node_id).unwrap_or_default();
    let (mutual, own): (Vec<_>, Vec<_>) = all_connections.into_iter().partition(|c| {
        sibling_set.contains(&c.src_uuid()) && sibling_set.contains(&c.target_uuid())
    });
    let mutual = mutual
        .into_iter()
        .filter(|c| {
            let key = (
                c.src_uuid(),
                c.src_port().to_string(),
                c.target_uuid(),
                c.target_port().to_string(),
            );
            seen_mutual.insert(key)
        })
        .collect();
    (own, mutual)
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
            NodeCacheItem::Analyzer(analyzer_dto) => {
                // Access info from DTO
                analyzer_dto
                    .info
                    .gui_position()
                    .unwrap_or_else(Point2::origin)
            }
        };

        corner.x = corner.x.min(pos.x);
        corner.y = corner.y.min(pos.y);
    }

    Ok(corner)
}

/// Replays every pasted group's own port map onto its freshly-created (still portless) copy.
///
/// Must process groups in `grouped_node_refs`'s own order (innermost/child groups before their
/// ancestors) rather than iterating `input_port_maps`/`output_port_maps` directly: a group's ports
/// are computed dynamically from its own port map, so mapping an ancestor's external port to a
/// nested group node only works once *that nested group's own* port map has already been rebuilt -
/// otherwise the nested group doesn't look like a valid mapping target yet and
/// `map_input_port`/`map_output_port` rejects it. `grouped_node_refs` already has exactly this
/// child-before-parent order, since `collect_optical_nodes_to_copy_recursive` only pushes a group's
/// own entry after its recursive call for its children has returned.
///
/// # Errors
///
/// Returns an error if a group's own port-map replay fails (e.g. an internal port name no longer
/// matching, which shouldn't happen given the maps were captured from the live original).
fn reconfigure_ports(
    scenery: &mut NodeGroup,
    grouped_node_refs: &[(Uuid, Vec<OpticRef>, bool)],
    input_port_maps: &HashMap<Uuid, PortMap>,
    output_port_maps: &HashMap<Uuid, PortMap>,
    node_id_link: &HashMap<Uuid, Uuid>,
    grouped_node_infos: &mut HashMap<Uuid, Vec<NodeInfo>>,
) -> Result<(), BackEndErrorResponse> {
    // output port maps
    for (old_group_id, _, _) in grouped_node_refs {
        let Some(output_port_map) = output_port_maps.get(old_group_id) else {
            continue;
        };
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
    // input port maps
    for (old_group_id, _, _) in grouped_node_refs {
        let Some(input_port_map) = input_port_maps.get(old_group_id) else {
            continue;
        };
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

    // set ports
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
                if let Some(ref_node) = node.as_any_mut().downcast_mut::<NodeReference>() {
                    let _ = ref_node.assign_reference(&referenced_node);
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
) -> AnalyzerItemDto {
    let old_pos = analyzer.gui_position().unwrap_or_default();
    let new_pos = Point2::new(old_pos.x + shift.x, old_pos.y + shift.y);
    let mut document = data.document.lock();

    // Add analyzer with new position, let opm_document generate the UUID
    let new_id = document.add_analyzer_with_position(
        analyzer.analyzer_type().clone(),
        Some((new_pos.x, new_pos.y)),
    );

    // Retrieve the newly created info struct
    let new_info = document.analyzers().get(&new_id).cloned().unwrap();
    drop(document);
    // Construct and return the DTO
    AnalyzerItemDto {
        id: new_id,
        info: new_info,
    }
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
        let node_id = node.uuid()?;

        let group_nodes_opt = {
            let guard = node.optical_ref.lock_opm()?;

            // Attempt to downcast the node reference to a NodeGroup
            guard.as_any().downcast_ref::<NodeGroup>().map(|group| {
                // These side-effects only run if the downcast was successful (Some)
                input_port_maps.insert(node_id, group.graph().port_map(&PortType::Input).clone());
                output_port_maps.insert(node_id, group.graph().port_map(&PortType::Output).clone());

                // Return the collected nodes, which will be wrapped in Some() by map()
                group.nodes().iter().copied().cloned().collect::<Vec<_>>()
            })
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
                let is_reference = is_reference_target(scenery, c.target_id);
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

    node_id_link.insert(old_node_id, new_node_ref.uuid()?);

    let parent_group_id = parent_group_id_or_self(scenery, old_node_id)?;

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

        let node_type = node.node_type().to_string();
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
        // Attempt to downcast the node mutably to a NodeReference
        if let Some(ref_node) = node.as_any_mut().downcast_mut::<NodeReference>() {
            ref_node.assign_reference(&referenced_node)?;
        } else {
            // Return an error if the node is not of type NodeReference,
            // replicating the behavior of the previous `as_refnode_mut()?` call.
            return Err(OpossumError::Other("Cannot cast to reference node".into()).into());
        }
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

/// Reconstructs the pre-existing external mappings that [`post_convert_nodes_to_group`] rerouted through
/// the freshly created `new_group_id`, reading them straight back out of the resulting document state so
/// the `ExtractGroup` undo command can restore them - no intermediate `pending` value required.
///
/// Because `new_group_id` is created **empty** and nothing else touches it during the conversion, every
/// entry in `group_id`'s own port map that points at `new_group_id` is - by construction - exactly one of
/// these reroutes: `group_id.external_name -> (new_group_id, group_internal_name)` on the parent, with
/// `new_group_id.group_internal_name -> (member_id, member_port)` on the group's own side. Boundary-sibling
/// edges never produce such an entry (they become a live connection in `group_id`, not a port-map entry
/// pointing into `new_group_id`), so this enumeration captures the reroutes and only the reroutes.
///
/// # Errors
///
/// Returns an error if `group_id`/`new_group_id` don't resolve, or a `group_id`-side entry points at a
/// `new_group_id` external name with no matching internal mapping (impossible by the invariant above).
fn reconstruct_rerouted_mappings(
    scenery: &NodeGroup,
    group_id: Uuid,
    new_group_id: Uuid,
) -> OpmResult<Vec<ReroutedMapping>> {
    let mut rerouted_mappings = Vec::new();
    for port_type in [PortType::Input, PortType::Output] {
        let exposed = scenery.with_group_node(group_id, |g| {
            g.graph()
                .port_map(&port_type)
                .assigned_ports_for_node(new_group_id)
        })?;
        for (external_name, group_internal_name) in exposed {
            let (member_id, member_port) = scenery
                .with_group_node(new_group_id, |g| {
                    g.graph()
                        .port_map(&port_type)
                        .get(&group_internal_name)
                        .cloned()
                })?
                .ok_or_else(|| {
                    OpossumError::Other(
                        "rerouted mapping's group-internal name has no matching member port".into(),
                    )
                })?;
            rerouted_mappings.push(ReroutedMapping {
                external_name,
                port_type,
                member_id,
                member_port,
                group_internal_name,
            });
        }
    }
    Ok(rerouted_mappings)
}

/// Convert the given nodes into a new subgroup within an existing group.
///
/// The request body must contain the ID of the source group (`group_id`) and a
/// list of node UUIDs (`nodes_to_convert`) that will be removed from the source
/// group and wrapped into a newly created group node.
///
/// Conceptually this is "create a brand-new empty child group inside `group_id`, then move the
/// selected nodes into it" - so it's implemented as exactly that, delegating the move to the shared
/// `relocate_nodes_in_document` (the same core `post_move_nodes` uses). That reuse is also what fixes
/// a node's pre-existing external port mapping on `group_id` (no live edge at this level - whatever
/// ultimately consumes it, a live edge or nothing, may be found arbitrarily far further out, e.g. when
/// this endpoint is called again on a group produced by an earlier call to it - see
/// `find_pre_existing_mapping_consumer`) being silently lost: the old two-step build-then-insert
/// approach never inspected `group_id`'s own port map at all, so a mapped node's export vanished the
/// moment it was deleted from `group_id`, with nothing to recreate it on the new group. The "collapse"
/// case the relocation also handles (the connection's other endpoint already lives in the destination)
/// is structurally unreachable here - the new group is always empty at creation - so every pre-existing
/// mapping on `group_id` is unconditionally a "reroute," recovered afterward from the resulting state by
/// `reconstruct_rerouted_mappings` for undo.
#[utoipa::path(
    tag = "operations",
    request_body(
        content = ConvertToGroupRequest,
        description = "Information about the parent group and the nodes to convert",
        content_type = "application/json"
    ),
    responses(
        (status = OK, description = "Nodes successfully converted to group; body reports the new group plus any connections rerouted as a side effect and which groups' port maps changed", body = ConvertToGroupResponse),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/convert_to_group")]
pub async fn post_convert_nodes_to_group(
    data: web::Data<AppState>,
    request: web::Json<ConvertToGroupRequest>,
) -> Result<Json<ConvertToGroupResponse>, BackEndErrorResponse> {
    // Unpack data from the request body
    let req = request.into_inner();
    let group_id = req.group_id;
    let nodes_to_convert = req.nodes_to_convert;
    let original_node_ids = nodes_to_convert.clone();

    // Only the new group's top-left position is needed here; the relocation below collects the node
    // refs it moves itself.
    let (_, pos) = collect_node_refs_and_pos(&data, &nodes_to_convert);

    // Undoing this conversion means extracting the new group's members back into `group_id` - see
    // `Command::ExtractGroup`'s docs for why capturing the group's own `OpticRef` is enough (its
    // internal members/connections are untouched, whether or not it's currently attached), and why it
    // separately needs `restore_connections` (every connection that touched a converted node before
    // grouping, in original member-uuid terms) rather than `external_connections` (which only makes
    // sense once the group itself exists again). Captured from the pre-move state, before the
    // relocation below rewires anything.
    let all_connections = collect_group_connections(&data, group_id)?;
    let split = split_sort_connections(&data, &all_connections, &nodes_to_convert);
    let restore_connections: Vec<ConnectInfo> = split
        .inside
        .into_iter()
        .chain(split.input)
        .chain(split.output)
        .collect();

    let mut document = data.document.lock();

    // Create the destination empty and attached first, before the move touches `group_id`'s port map -
    // this is what lets the relocation see (and preserve, by rerouting) a pre-existing mapping on
    // `group_id`, which requires a real destination to reroute into.
    let new_group_id = document
        .scenery_mut()
        .with_group_node_mut(group_id, |g| g.add_node(NodeGroup::new("new group")))??;

    // Convert-to-group *is* a move: relocate the selected nodes out of `group_id` into the freshly
    // created child, reusing the exact same machinery as `post_move_nodes` (boundary edges rerouted,
    // pre-existing external mappings preserved, references followed).
    let outcome =
        relocate_nodes_in_document(&mut document, group_id, new_group_id, &original_node_ids)?;

    // The undo command needs each pre-existing mapping the move rerouted through the new group. Since the
    // new group was just created empty, they're exactly `group_id`'s port-map entries now pointing at it -
    // read them back out of the resulting state instead of threading them out of the move itself.
    let rerouted_mappings =
        reconstruct_rerouted_mappings(document.scenery(), group_id, new_group_id)?;

    let mut port_map_groups_changed = outcome.preserved.port_map_groups_changed;
    port_map_groups_changed.push(new_group_id);
    port_map_groups_changed.sort();
    port_map_groups_changed.dedup();

    let group_ref = document.scenery().node_recursive(new_group_id)?.0;
    data.push_undo(Command::ExtractGroup(GroupConversion {
        parent_group_id: group_id,
        group: group_ref,
        member_ids: original_node_ids,
        external_connections: outcome
            .preserved
            .new_connections
            .iter()
            .map(|(_, c)| c.clone())
            .collect(),
        restore_connections,
        rerouted_mappings,
        // Refresh every group a reroute touched (not just the parent) on undo/redo of this conversion.
        affected_groups: port_map_groups_changed.clone(),
    }));

    drop(document);

    // Create the nodeinfo struct for the GUI
    let new_group_node_info = create_new_group_node_info(&data, new_group_id, pos)?;

    Ok(Json(ConvertToGroupResponse {
        new_group: new_group_node_info,
        new_connections: outcome.preserved.new_connections,
        removed_connections: outcome.removed_connections,
        port_map_groups_changed,
        removed_port_mappings: outcome.preserved.removed_port_mappings,
    }))
}

/// Move the given nodes from one group into another group.
///
/// All specified nodes will be removed from the source group and inserted into
/// the target group, including their internal connections. Any connection to a sibling left behind in
/// the source group (or an external connection depending on a port mapping of a moved node) can't be
/// preserved across the move - a direct edge requires both endpoints to share a graph - so it's
/// preserved instead - a fresh port mapping is created on the destination group and the connection is
/// rerouted through it (or, if the connection's other endpoint already lives in the destination - what
/// undoing a previous move looks like - reconnected directly). What changed is returned so the GUI can
/// reflect it immediately; undo is a plain reverse move, since re-running this same logic from the
/// document's live state is enough to correctly unwind whatever this call set up.
#[utoipa::path(
    tag = "operations",
    request_body(
        content = MoveNodesRequest,
        description = "Information about the source group, target group, and nodes to move",
        content_type = "application/json"
    ),
    responses(
        (status = OK, description = "Nodes successfully transferred to group; body reports any connections rerouted as a side effect and which groups' port maps changed", body = MoveNodesResponse),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/move_nodes")]
pub async fn post_move_nodes(
    data: web::Data<AppState>,
    request: web::Json<MoveNodesRequest>,
) -> Result<Json<MoveNodesResponse>, BackEndErrorResponse> {
    // Unpack data from the request body
    let req = request.into_inner();
    let from_group_id = req.source_group_id;
    let drop_group_id = req.target_group_id;
    let original_node_ids = req.nodes_to_move;

    let mut document = data.document.lock();

    // Relocate the nodes into the drop group, preserving each node's uuid - the shared core also driven by
    // the cut operation and by move undo/redo (`apply_move_nodes`).
    let outcome = relocate_nodes_in_document(
        &mut document,
        from_group_id,
        drop_group_id,
        &original_node_ids,
    )?;

    let mut port_map_groups_changed = outcome.preserved.port_map_groups_changed;
    port_map_groups_changed.push(drop_group_id);
    port_map_groups_changed.sort();
    port_map_groups_changed.dedup();

    // The tab undo/redo should focus: the drag's outer context (the lowest common ancestor of source and
    // target), so the view stays there instead of being pulled into the group - the change is visible in
    // the outer tab either direction. Same value for the move and its reverse (see `MoveNodes`).
    let focus_group_id =
        lowest_common_ancestor_group(document.scenery(), from_group_id, drop_group_id)?;

    // Carry the touched-group set into the undo command so undo/redo refreshes every affected tab, not
    // just source and target.
    data.push_undo(Command::MoveNodes(MoveNodes {
        request: MoveNodesRequest {
            source_group_id: drop_group_id,
            target_group_id: from_group_id,
            nodes_to_move: original_node_ids,
        },
        affected_groups: port_map_groups_changed.clone(),
        focus_group_id,
    }));

    drop(document);
    Ok(Json(MoveNodesResponse {
        new_connections: outcome.preserved.new_connections,
        removed_connections: outcome.removed_connections,
        port_map_groups_changed,
        removed_port_mappings: outcome.preserved.removed_port_mappings,
    }))
}

pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(post_copy_nodes);
    cfg.service(post_paste_nodes);
    cfg.service(post_cut_nodes);

    cfg.service(post_convert_nodes_to_group);
    cfg.service(post_move_nodes);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::document::{redo_document, undo_document};
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use opossum_core::{meter, nodes::Dummy};

    /// Regression test for the redo bug this branch's original cut mechanism had: create a node and a
    /// reference to it, cut+paste the node in the *same* scenery, undo, then redo. The old duplicate-based
    /// cut minted a new uuid for the pasted copy and retargeted the reference at it; on redo it deleted the
    /// original, whose reference-cascade swept the reference node away *before* the retarget step ran, so
    /// redo failed with "node with given uuid does not exist". The UUID-preserving cut makes a same-group
    /// cut a pure reposition - no relocation, no deletion, no cascade - so the node keeps its uuid, the
    /// reference stays valid without ever being touched, and undo *and* redo both succeed.
    #[actix_web::test]
    async fn test_cut_paste_node_with_reference_undo_redo_same_group() {
        use opossum_core::nodes::Dummy;

        let app_state = Data::new(AppState::default());
        let (root_id, node_a, ref_id) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();
            let node_a = scenery.add_node(Dummy::default()).unwrap();
            let node_a_ref = scenery.node_recursive(node_a).unwrap().0;
            let node_reference = NodeReference::from_node(&node_a_ref).unwrap();
            let ref_id = scenery.add_node(node_reference).unwrap();
            (root_id, node_a, ref_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_copy_nodes)
                .service(post_cut_nodes)
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        // Copy A, then cut it into the same (root) scenery at a new position.
        let mut nodes_to_copy = HashSet::new();
        nodes_to_copy.insert(node_a);
        let req = test::TestRequest::post()
            .uri("/copy_nodes")
            .set_json(&nodes_to_copy)
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );

        let req = test::TestRequest::post()
            .uri("/cut_nodes")
            .set_json(&(root_id, (500.0, 500.0)))
            .to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);

        {
            let document = app_state.document.lock();
            assert!(
                document.scenery().node_recursive(node_a).is_ok(),
                "the cut node must keep its original uuid (a move, not a duplicate)"
            );
            let node_count = document
                .scenery()
                .with_group_node(root_id, |g| g.nodes().len())
                .unwrap();
            assert_eq!(
                node_count, 2,
                "a cut must not create a duplicate node - only A and its reference remain"
            );
            let ref_target = document
                .scenery()
                .with_node_attr(ref_id, |attr| {
                    attr.properties().get("reference id").cloned()
                })
                .unwrap()
                .unwrap();
            assert_eq!(
                ref_target,
                Proptype::Uuid(node_a),
                "the reference still points at A's unchanged uuid after the cut"
            );
        }

        // Undo, then redo - the redo is what used to fail with a 400.
        assert_eq!(
            app.call(test::TestRequest::post().uri("/undo").to_request())
                .await
                .unwrap()
                .status(),
            StatusCode::OK,
            "undo of the cut must succeed"
        );
        assert_eq!(
            app.call(test::TestRequest::post().uri("/redo").to_request())
                .await
                .unwrap()
                .status(),
            StatusCode::OK,
            "redo of the cut must succeed (regression: used to be 400 'node with given uuid does not exist')"
        );

        let document = app_state.document.lock();
        assert!(
            document.scenery().node_recursive(node_a).is_ok(),
            "A must still exist under its original uuid after redo"
        );
        assert!(
            document.scenery().node_recursive(ref_id).is_ok(),
            "the reference node must survive undo+redo"
        );
        let ref_target = document
            .scenery()
            .with_node_attr(ref_id, |attr| {
                attr.properties().get("reference id").cloned()
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            ref_target,
            Proptype::Uuid(node_a),
            "the reference must still resolve to A's unchanged uuid after redo"
        );
    }

    /// Companion to the same-group case: cutting a node *out of a group* into the root is a real
    /// relocation (not just a reposition), yet still preserves the node's uuid. A reference to the moved
    /// node - here living in the root, one level up - must keep resolving to it in its new location across
    /// the cut and across undo/redo, never dangling or being cascade-deleted.
    #[actix_web::test]
    async fn test_cut_paste_node_with_reference_undo_redo_cross_group() {
        use opossum_core::nodes::{Dummy, NodeGroup};

        let app_state = Data::new(AppState::default());
        let (root_id, group_id, node_a, ref_id) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();

            let mut group = NodeGroup::new("inner group");
            let node_a = group.add_node(Dummy::default()).unwrap();
            let group_id = scenery.add_node(group).unwrap();

            let node_a_ref = scenery.node_recursive(node_a).unwrap().0;
            let node_reference = NodeReference::from_node(&node_a_ref).unwrap();
            let ref_id = scenery.add_node(node_reference).unwrap();

            (root_id, group_id, node_a, ref_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_copy_nodes)
                .service(post_cut_nodes)
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        let mut nodes_to_copy = HashSet::new();
        nodes_to_copy.insert(node_a);
        let req = test::TestRequest::post()
            .uri("/copy_nodes")
            .set_json(&nodes_to_copy)
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );

        // Cut A out of the group and into the root scenery.
        let req = test::TestRequest::post()
            .uri("/cut_nodes")
            .set_json(&(root_id, (500.0, 500.0)))
            .to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);

        let node_a_in = |group: Uuid| {
            app_state
                .document
                .lock()
                .scenery()
                .with_group_node(group, |g| {
                    g.nodes()
                        .iter()
                        .filter_map(|n| n.uuid().ok())
                        .any(|id| id == node_a)
                })
                .unwrap()
        };

        assert!(
            node_a_in(root_id),
            "A must have moved into the root under its original uuid"
        );
        assert!(!node_a_in(group_id), "A must no longer be in the group");
        assert!(
            app_state
                .document
                .lock()
                .scenery()
                .node_recursive(ref_id)
                .is_ok(),
            "the reference node must survive the cross-group cut"
        );

        // Undo puts A back in the group; redo moves it out again - both must succeed.
        assert_eq!(
            app.call(test::TestRequest::post().uri("/undo").to_request())
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert!(node_a_in(group_id), "undo must move A back into the group");
        assert!(!node_a_in(root_id), "undo must remove A from the root");

        assert_eq!(
            app.call(test::TestRequest::post().uri("/redo").to_request())
                .await
                .unwrap()
                .status(),
            StatusCode::OK,
            "redo of the cross-group cut must succeed"
        );
        assert!(
            node_a_in(root_id),
            "redo must move A back out into the root"
        );

        let document = app_state.document.lock();
        let ref_target = document
            .scenery()
            .with_node_attr(ref_id, |attr| {
                attr.properties().get("reference id").cloned()
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            ref_target,
            Proptype::Uuid(node_a),
            "the reference must still resolve to A's unchanged uuid after the cross-group redo"
        );
    }

    /// Regression test for the bug where pasting a *group* whose members are internally connected
    /// could 400 on undo ("node with given uuid does not exist"), or silently lose the members'
    /// internal connection on redo. Root cause: the undo batch used to construct one `RemoveNode`
    /// per pasted node at *every* nesting level - both the group itself and, redundantly, each of
    /// its members - even though removing the group alone already captures/restores its entire
    /// internal subtree via its own `OpticRef`. Since `Command::Batch` applies in a
    /// non-deterministic order (derived from `HashMap` iteration), the member's own redundant
    /// `RemoveNode` could target a uuid its ancestor's `RemoveNode` already cascaded away (undo
    /// 400s), or run first and sever the internal connection before the group's own command even
    /// applies (redo can't restore it, since a nested `AddNode` always passes an empty
    /// `connections` list). Builds `G { A -> B }`, copies (not cuts, to isolate this from the
    /// cut-deletion path) and pastes `G` itself, and asserts a single undo succeeds and a
    /// following redo restores the group with its internal `A -> B` connection intact.
    #[actix_web::test]
    async fn test_undo_redo_paste_group_preserves_internal_connection() {
        use opossum_core::nodes::{Dummy, NodeGroup};

        let app_state = Data::new(AppState::default());
        let (root_id, group_id) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();

            let mut group = NodeGroup::new("inner group");
            let node_a = group.add_node(Dummy::default()).unwrap();
            let node_b = group.add_node(Dummy::default()).unwrap();
            group
                .connect_nodes(node_a, "output_1", node_b, "input_1", meter!(0.1))
                .unwrap();
            let group_id = scenery.add_node(group).unwrap();

            (root_id, group_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_copy_nodes)
                .service(post_paste_nodes)
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        // Copy G itself (not its members) - this is what makes `collect_optical_nodes_to_copy_recursive`
        // recurse into it and populate a *nested* entry in `grouped_node_infos`, the shape that
        // triggered the redundant-`RemoveNode` bug.
        let mut nodes_to_copy = HashSet::new();
        nodes_to_copy.insert(group_id);
        let req = test::TestRequest::post()
            .uri("/copy_nodes")
            .set_json(&nodes_to_copy)
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );

        let req = test::TestRequest::post()
            .uri("/paste_nodes")
            .set_json(&(root_id, (500.0, 500.0)))
            .to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);

        let pasted_group_id = {
            let document = app_state.document.lock();
            document
                .scenery()
                .with_group_node(root_id, |g| {
                    g.nodes()
                        .iter()
                        .filter_map(|n| n.uuid().ok())
                        .find(|id| *id != group_id)
                })
                .unwrap()
                .expect("a pasted duplicate group must exist")
        };

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "undoing the paste of a group with internally-connected members must not error"
        );
        {
            let document = app_state.document.lock();
            assert!(
                document.scenery().node_recursive(pasted_group_id).is_err(),
                "the pasted duplicate must be gone after undo"
            );
        }

        let req = test::TestRequest::post().uri("/redo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "redo must not error");

        let document = app_state.document.lock();
        assert!(
            document.scenery().node_recursive(pasted_group_id).is_ok(),
            "the pasted group must be restored by redo"
        );
        let connections = document
            .scenery()
            .with_group_node(pasted_group_id, NodeGroup::connections)
            .unwrap();
        assert_eq!(
            connections.len(),
            1,
            "the pasted group's internal A -> B connection must survive undo+redo"
        );
    }

    /// Regression test for the bug where redoing a paste of two mutually-connected flat (non-grouped)
    /// sibling nodes lost the connection between them. Root cause: `post_paste_nodes` built each
    /// top-level pasted node's `Command::RemoveNode` with an empty `connections` field, so undo's
    /// resulting `AddNode` (the command `Command::Batch` reverses onto the redo stack) never carried
    /// the pasted pair's connection - `apply_remove_node` only forwards whatever `connections` its
    /// `RemoveNode` snapshot already had. Builds `G { A -> B }`, copies A and B individually (not `G`,
    /// which would hide the bug behind the group's own `OpticRef`-embedded internal edge - see
    /// `test_undo_redo_paste_group_preserves_internal_connection` above), pastes them (not cut, to
    /// isolate this from `perform_cut`'s own already-correct mutual-connection handling) into the root
    /// scenery, and asserts a single undo succeeds and a following redo restores both pasted nodes
    /// *and* the connection between them.
    #[actix_web::test]
    async fn test_undo_redo_paste_mutually_connected_nodes_preserves_connection() {
        use opossum_core::nodes::{Dummy, NodeGroup};

        let app_state = Data::new(AppState::default());
        let (root_id, group_id, node_a, node_b) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();

            let mut group = NodeGroup::new("inner group");
            let node_a = group.add_node(Dummy::default()).unwrap();
            let node_b = group.add_node(Dummy::default()).unwrap();
            group
                .connect_nodes(node_a, "output_1", node_b, "input_1", meter!(0.1))
                .unwrap();
            let group_id = scenery.add_node(group).unwrap();

            (root_id, group_id, node_a, node_b)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_copy_nodes)
                .service(post_paste_nodes)
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        let mut nodes_to_copy = HashSet::new();
        nodes_to_copy.insert(node_a);
        nodes_to_copy.insert(node_b);
        let req = test::TestRequest::post()
            .uri("/copy_nodes")
            .set_json(&nodes_to_copy)
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );

        // Paste (not cut) both A and B as two flat siblings connected to each other, directly into
        // the root scenery.
        let req = test::TestRequest::post()
            .uri("/paste_nodes")
            .set_json(&(root_id, (500.0, 500.0)))
            .to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);

        let pasted_ids: Vec<Uuid> = {
            let document = app_state.document.lock();
            document
                .scenery()
                .with_group_node(root_id, |g| {
                    g.nodes()
                        .iter()
                        .filter_map(|n| n.uuid().ok())
                        .filter(|id| *id != group_id)
                        .collect::<Vec<_>>()
                })
                .unwrap()
        };
        assert_eq!(pasted_ids.len(), 2, "both A and B must have been pasted");

        let req = test::TestRequest::post().uri("/undo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "undoing the paste must not error"
        );
        {
            let document = app_state.document.lock();
            for id in &pasted_ids {
                assert!(
                    document.scenery().node_recursive(*id).is_err(),
                    "the pasted duplicate must be gone after undo"
                );
            }
        }

        let req = test::TestRequest::post().uri("/redo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "redo must not error"
        );

        let document = app_state.document.lock();
        for id in &pasted_ids {
            assert!(
                document.scenery().node_recursive(*id).is_ok(),
                "the pasted duplicate must be restored by redo"
            );
        }
        let connections = document
            .scenery()
            .with_group_node(root_id, NodeGroup::connections)
            .unwrap();
        let pasted_connection_count = connections
            .iter()
            .filter(|c| pasted_ids.contains(&c.src_id) && pasted_ids.contains(&c.target_id))
            .count();
        assert_eq!(
            pasted_connection_count, 1,
            "the connection between the two redo-restored pasted nodes must survive redo"
        );
    }

    /// Regression test for the bug where pasting a group that itself contains a nested group with its
    /// own port map failed with `OpticGroup:node to be mapped is not an input_1/output_1 node of the
    /// group`. Root cause: `reconfigure_ports` replayed the pasted subtree's collected port maps by
    /// iterating a plain `HashMap`, in arbitrary order - so an outer group's port map (which maps one
    /// of its own external ports to a *nested* group node) could be replayed before that nested
    /// group's own port map had been rebuilt, at which point the nested group didn't yet look like a
    /// valid mapping target. Builds `root -> G1 -> G2 -> [A, B]`, where `G2` maps `A`'s input and `B`'s
    /// output to its own external names, and `G1` maps `G2`'s external names to its own - mirroring the
    /// reported repro exactly (a group inside a group, both with port maps). Copies and pastes `G1` and
    /// asserts the paste succeeds and the pasted copy's external ports match the original.
    #[actix_web::test]
    async fn test_paste_doubly_nested_group_with_port_maps_at_both_levels() {
        use opossum_core::{
            nodes::{Dummy, NodeGroup},
            prelude::PortType,
        };

        let app_state = Data::new(AppState::default());
        let (root_id, g1_id) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();

            let mut g2 = NodeGroup::new("G2");
            let node_a = g2.add_node(Dummy::default()).unwrap();
            let node_b = g2.add_node(Dummy::default()).unwrap();
            g2.connect_nodes(node_a, "output_1", node_b, "input_1", meter!(0.1))
                .unwrap();
            g2.map_input_port(node_a, "input_1", "g2_ext_in").unwrap();
            g2.map_output_port(node_b, "output_1", "g2_ext_out")
                .unwrap();

            let mut g1 = NodeGroup::new("G1");
            let g2_id = g1.add_node(g2).unwrap();
            g1.map_input_port(g2_id, "g2_ext_in", "g1_ext_in").unwrap();
            g1.map_output_port(g2_id, "g2_ext_out", "g1_ext_out")
                .unwrap();

            let g1_id = scenery.add_node(g1).unwrap();

            (root_id, g1_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_copy_nodes)
                .service(post_paste_nodes),
        )
        .await;

        let mut nodes_to_copy = HashSet::new();
        nodes_to_copy.insert(g1_id);
        let req = test::TestRequest::post()
            .uri("/copy_nodes")
            .set_json(&nodes_to_copy)
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );

        let req = test::TestRequest::post()
            .uri("/paste_nodes")
            .set_json(&(root_id, (500.0, 500.0)))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "pasting a group containing a nested, port-mapped group must not error"
        );

        let pasted_g1_id = {
            let document = app_state.document.lock();
            document
                .scenery()
                .with_group_node(root_id, |g| {
                    g.nodes()
                        .iter()
                        .filter_map(|n| n.uuid().ok())
                        .find(|id| *id != g1_id)
                })
                .unwrap()
                .expect("a pasted duplicate of G1 must exist")
        };

        let document = app_state.document.lock();
        let (input_names, output_names) = document
            .scenery()
            .with_group_node(pasted_g1_id, |g| {
                (
                    g.ports().names(&PortType::Input),
                    g.ports().names(&PortType::Output),
                )
            })
            .unwrap();
        assert!(
            input_names.contains(&"g1_ext_in".to_string()),
            "the pasted G1's external input port must be restored"
        );
        assert!(
            output_names.contains(&"g1_ext_out".to_string()),
            "the pasted G1's external output port must be restored"
        );
    }

    /// Regression test for the behavior change requested after live-testing the original
    /// disconnect-based fix: dragging a connected node into a different group should *preserve* its
    /// connections - rerouting through a freshly created port mapping on the destination - rather than
    /// disconnecting them. Builds `from_group { moved_node, sibling, to_group }` (`to_group` a nested
    /// subgroup, matching the real drag-and-drop UI constraint that a node can only ever be dropped into a
    /// group that is a direct child of the level it's currently viewed at) with `sibling -> moved_node`
    /// internal, and `from_group` also mapping `moved_node`'s output externally, connected to sibling `S`
    /// in root; moves `moved_node` into `to_group`; asserts both connections are still live right after
    /// the move (rerouted, not dropped) and `S`'s connection is completely untouched; undo restores
    /// `moved_node` to `from_group` with both connections reconnected directly again and `to_group`'s port
    /// map back to empty (no leftover mapping); redo re-applies the reroute and a second undo lands back
    /// at the exact pristine state again, proving the cycle is stable.
    #[actix_web::test]
    async fn test_move_node_preserves_connections_via_auto_mapping() {
        use opossum_core::{
            nodes::{Dummy, NodeGroup},
            prelude::PortType,
        };

        let app_state = Data::new(AppState::default());
        let (from_group_id, to_group_id, moved_node, sibling, s) = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();

            let mut from_group = NodeGroup::new("from group");
            let moved_node = from_group.add_node(Dummy::default()).unwrap();
            let sibling = from_group.add_node(Dummy::default()).unwrap();
            from_group
                .connect_nodes(sibling, "output_1", moved_node, "input_1", meter!(0.1))
                .unwrap();
            from_group
                .map_output_port(moved_node, "output_1", "ext_out_1")
                .unwrap();
            let to_group_id = from_group.add_node(NodeGroup::new("to group")).unwrap();
            let from_group_id = scenery.add_node(from_group).unwrap();

            let s = scenery.add_node(Dummy::default()).unwrap();
            scenery
                .connect_nodes(from_group_id, "ext_out_1", s, "input_1", meter!(0.1))
                .unwrap();

            (from_group_id, to_group_id, moved_node, sibling, s)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_move_nodes)
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        // Asserts the state matches "moved_node currently lives in to_group_id, with both connections
        // preserved through it" - used both right after the initial move and again after redo.
        let assert_moved_state = |app_state: &Data<AppState>| {
            let document = app_state.document.lock();
            assert!(
                document
                    .scenery()
                    .node_recursive(moved_node)
                    .is_ok_and(|(_, parent)| parent == to_group_id),
                "moved_node must live in to_group"
            );
            // Boundary connection: sibling -> moved_node must now be rerouted as sibling ->
            // to_group.<generated>, still live inside from_group's own graph, with to_group exposing a
            // fresh input mapping for moved_node.
            let from_connections = document
                .scenery()
                .with_group_node(from_group_id, NodeGroup::connections)
                .unwrap();
            assert!(
                !from_connections
                    .iter()
                    .any(|c| c.src_id == sibling && c.target_id == moved_node),
                "the old direct sibling -> moved_node connection must be gone"
            );
            assert!(
                from_connections
                    .iter()
                    .any(|c| c.src_id == sibling && c.target_id == to_group_id),
                "sibling must now connect to to_group instead"
            );
            let to_group_input_mapped = document
                .scenery()
                .with_group_node(to_group_id, |g| {
                    !g.graph()
                        .port_map(&PortType::Input)
                        .assigned_ports_for_node(moved_node)
                        .is_empty()
                })
                .unwrap();
            assert!(
                to_group_input_mapped,
                "to_group must expose a fresh mapping for moved_node's input port"
            );

            // Pre-existing mapping: from_group's own "ext_out_1" entry must now route through to_group
            // instead of directly at moved_node - the S connection itself is untouched throughout.
            let ext_out_target = document
                .scenery()
                .with_group_node(from_group_id, |g| {
                    g.graph()
                        .port_map(&PortType::Output)
                        .get("ext_out_1")
                        .cloned()
                })
                .unwrap();
            assert!(
                ext_out_target.is_some_and(|(id, _)| id == to_group_id),
                "from_group's ext_out_1 mapping must now route through to_group"
            );
            let to_group_output_mapped = document
                .scenery()
                .with_group_node(to_group_id, |g| {
                    !g.graph()
                        .port_map(&PortType::Output)
                        .assigned_ports_for_node(moved_node)
                        .is_empty()
                })
                .unwrap();
            assert!(
                to_group_output_mapped,
                "to_group must expose a fresh mapping for moved_node's output port"
            );
            let root_connections = document.scenery().graph().connections();
            assert!(
                root_connections
                    .iter()
                    .any(|c| c.src_id == from_group_id && c.target_id == s),
                "the S connection to from_group's ext_out_1 must be completely untouched"
            );
        };

        // Asserts the pristine, pre-move state - used both for the original setup (implicitly) and after
        // each undo.
        let assert_pristine_state = |app_state: &Data<AppState>| {
            let document = app_state.document.lock();
            assert!(
                document
                    .scenery()
                    .node_recursive(moved_node)
                    .is_ok_and(|(_, parent)| parent == from_group_id),
                "moved_node must be back in from_group"
            );
            let from_connections = document
                .scenery()
                .with_group_node(from_group_id, NodeGroup::connections)
                .unwrap();
            assert!(
                from_connections
                    .iter()
                    .any(|c| c.src_id == sibling && c.target_id == moved_node),
                "the direct sibling -> moved_node connection must be restored"
            );
            let ext_out_target = document
                .scenery()
                .with_group_node(from_group_id, |g| {
                    g.graph()
                        .port_map(&PortType::Output)
                        .get("ext_out_1")
                        .cloned()
                })
                .unwrap();
            assert_eq!(
                ext_out_target,
                Some((moved_node, "output_1".to_string())),
                "from_group's ext_out_1 mapping must resolve directly to moved_node again"
            );
            let to_group_empty = document
                .scenery()
                .with_group_node(to_group_id, |g| {
                    g.graph().port_map(&PortType::Input).is_empty()
                        && g.graph().port_map(&PortType::Output).is_empty()
                })
                .unwrap();
            assert!(
                to_group_empty,
                "to_group must have no leftover mapping entries after undo"
            );
            let root_connections = document.scenery().graph().connections();
            assert!(
                root_connections
                    .iter()
                    .any(|c| c.src_id == from_group_id && c.target_id == s),
                "the S connection to from_group's ext_out_1 must still be untouched"
            );
        };

        let move_req = || {
            test::TestRequest::post()
                .uri("/move_nodes")
                .set_json(&MoveNodesRequest {
                    source_group_id: from_group_id,
                    target_group_id: to_group_id,
                    nodes_to_move: vec![moved_node],
                })
                .to_request()
        };

        let resp = app.call(move_req()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let response: MoveNodesResponse = test::read_body_json(resp).await;
        assert_eq!(
            response.new_connections.len(),
            1,
            "only the boundary connection produces a new edge - the pre-existing mapping is retargeted \
             in place, with no edge change"
        );
        assert_eq!(response.removed_connections.len(), 1);
        assert!(
            response.removed_port_mappings.is_empty(),
            "nothing collapses on the initial move - both cases are fresh reroutes"
        );
        assert_moved_state(&app_state);

        let req = test::TestRequest::post().uri("/undo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "undo must not error"
        );
        assert_pristine_state(&app_state);

        let req = test::TestRequest::post().uri("/redo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "redo must not error"
        );
        assert_moved_state(&app_state);

        let req = test::TestRequest::post().uri("/undo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "the second undo must not error"
        );
        assert_pristine_state(&app_state);
    }

    /// Regression test for the bug where undoing/redoing a move whose selection contained a node *and* a
    /// reference to it errored with "node ... does not exist". The forward `post_move_nodes` deletes the
    /// moved nodes cascade-aware, but `apply_move_nodes` (the undo/redo path) used a plain per-id loop:
    /// deleting the node cascaded the reference away, then deleting the (now-gone) reference failed. Moves
    /// `{A, reference-to-A}` (both in the source group) into another group and asserts undo *and* redo
    /// both succeed and relocate both nodes.
    #[actix_web::test]
    async fn test_move_nodes_undo_redo_handles_internal_reference_cascade() {
        use opossum_core::nodes::{Dummy, NodeGroup, NodeReference};

        let app_state = Data::new(AppState::default());
        let (from_group_id, to_group_id, node_a, ref_r) = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();

            let mut from_group = NodeGroup::new("from group");
            let node_a = from_group.add_node(Dummy::default()).unwrap();
            let node_a_ref = from_group.node_recursive(node_a).unwrap().0;
            let ref_r = from_group
                .add_node(NodeReference::from_node(&node_a_ref).unwrap())
                .unwrap();
            let to_group_id = from_group.add_node(NodeGroup::new("to group")).unwrap();
            let from_group_id = scenery.add_node(from_group).unwrap();
            (from_group_id, to_group_id, node_a, ref_r)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_move_nodes)
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        // Forward move of {A, reference-to-A} together (the forward path is already cascade-aware).
        let req = test::TestRequest::post()
            .uri("/move_nodes")
            .set_json(&MoveNodesRequest {
                source_group_id: from_group_id,
                target_group_id: to_group_id,
                nodes_to_move: vec![node_a, ref_r],
            })
            .to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);

        // The regression: undo must not error on the internal cascade (delete A cascades R, then R must
        // not be deleted again), and must relocate both nodes back to the source group.
        let req = test::TestRequest::post().uri("/undo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "undo of a move containing a node and a reference to it must not error"
        );
        {
            let document = app_state.document.lock();
            assert!(
                document
                    .scenery()
                    .node_recursive(node_a)
                    .is_ok_and(|(_, p)| p == from_group_id),
                "node A must be back in the source group after undo"
            );
            assert!(
                document
                    .scenery()
                    .node_recursive(ref_r)
                    .is_ok_and(|(_, p)| p == from_group_id),
                "the reference must be back in the source group after undo"
            );
        }

        // Redo must likewise not error and re-relocate both nodes to the target group.
        let req = test::TestRequest::post().uri("/redo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "redo of the same move must not error either"
        );
        {
            let document = app_state.document.lock();
            assert!(
                document
                    .scenery()
                    .node_recursive(node_a)
                    .is_ok_and(|(_, p)| p == to_group_id),
                "node A must be in the target group again after redo"
            );
            assert!(
                document
                    .scenery()
                    .node_recursive(ref_r)
                    .is_ok_and(|(_, p)| p == to_group_id),
                "the reference must be in the target group again after redo"
            );
        }
    }

    /// Regression test for the bug where converting nodes to a group *deleted* an external reference node
    /// pointing at one of them. Grouping is a relocation - the grouped node keeps its uuid and the
    /// reference's `Weak` stays valid - but the removal step used the cascading `delete_node`, which swept
    /// the referrer away (and nothing captured it for undo, so undo couldn't restore it either). Builds
    /// `root { A, B, ref -> A }`, converts `{A, B}` into a new group, and asserts the reference survives
    /// *and still resolves to A* (its mirrored ports are non-empty) across the convert, its undo, and its
    /// redo.
    #[actix_web::test]
    async fn test_convert_nodes_to_group_keeps_external_reference_alive() {
        use opossum_core::nodes::{Dummy, NodeReference};

        let app_state = Data::new(AppState::default());
        let (root_id, node_a, node_b, ref_r) = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();
            let root_id = scenery.node_attr().uuid();
            let node_a = scenery.add_node(Dummy::default()).unwrap();
            let node_b = scenery.add_node(Dummy::default()).unwrap();
            let node_a_ref = scenery.node_recursive(node_a).unwrap().0;
            let ref_r = scenery
                .add_node(NodeReference::from_node(&node_a_ref).unwrap())
                .unwrap();
            (root_id, node_a, node_b, ref_r)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_convert_nodes_to_group)
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        // The reference must still exist at the root and still resolve to A - a live `NodeReference` mirrors
        // its target's ports, so a Dummy target gives it a non-empty output port set; a dropped reference
        // would be gone entirely.
        let assert_reference_resolves = |app_state: &Data<AppState>| {
            let document = app_state.document.lock();
            let (ref_node, parent) = document
                .scenery()
                .node_recursive(ref_r)
                .expect("the reference node must still exist");
            assert_eq!(parent, root_id, "the reference must stay at the root");
            let ports = ref_node.optical_ref.lock_opm().unwrap().ports();
            assert!(
                !ports.names(&PortType::Output).is_empty(),
                "the reference must still resolve to A (non-empty mirrored ports)"
            );
        };

        let req = test::TestRequest::post()
            .uri("/convert_to_group")
            .set_json(&ConvertToGroupRequest {
                group_id: root_id,
                nodes_to_convert: vec![node_a, node_b],
            })
            .to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);
        assert_reference_resolves(&app_state);

        let req = test::TestRequest::post().uri("/undo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "undo must not error"
        );
        assert_reference_resolves(&app_state);

        let req = test::TestRequest::post().uri("/redo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "redo must not error"
        );
        assert_reference_resolves(&app_state);
    }

    /// `lowest_common_ancestor_group` must return the outer/drag-origin tab for every move topology, so a
    /// move's jump target is direction-stable. Builds `root { G1 { A, G2 }, G3 }` and checks: an into-group
    /// move (`G1` <-> `G2`) resolves to the outer `G1`; a move between differently-nested branches
    /// (`G2` <-> `G3`) resolves to their shared root; and a move out of the root resolves to the root.
    #[actix_web::test]
    async fn test_lowest_common_ancestor_group_picks_the_outer_tab() {
        use opossum_core::nodes::{Dummy, NodeGroup};

        let app_state = Data::new(AppState::default());
        let mut document = app_state.document.lock();
        let scenery = document.scenery_mut();
        let root_id = scenery.node_attr().uuid();
        let mut g1 = NodeGroup::new("G1");
        let _a = g1.add_node(Dummy::default()).unwrap();
        let g2_id = g1.add_node(NodeGroup::new("G2")).unwrap();
        let g1_id = scenery.add_node(g1).unwrap();
        let g3_id = scenery.add_node(NodeGroup::new("G3")).unwrap();

        // Into-group and out-of-group both land on the outer group G1 (which contains G2).
        assert_eq!(
            lowest_common_ancestor_group(scenery, g1_id, g2_id).unwrap(),
            g1_id
        );
        assert_eq!(
            lowest_common_ancestor_group(scenery, g2_id, g1_id).unwrap(),
            g1_id
        );
        // G2 (under G1) and G3 (under root) share only the root.
        assert_eq!(
            lowest_common_ancestor_group(scenery, g2_id, g3_id).unwrap(),
            root_id
        );
        // A move that starts at the root stays at the root.
        assert_eq!(
            lowest_common_ancestor_group(scenery, root_id, g1_id).unwrap(),
            root_id
        );
    }

    /// Regression test for the bug where undo then redo of a move-into-group jumped *into* the group on
    /// redo. `MoveNodes::jump_target` used `target_group_id` - the destination of whichever direction is
    /// applied - which is the outer group on undo but the inner group on redo. It now uses a
    /// direction-stable `focus_group_id` (the lowest common ancestor). Moves a node from the root into a
    /// child group and asserts the pushed undo command *and* the re-inverted redo command report the same
    /// jump-target tab, equal to the outer root.
    #[actix_web::test]
    async fn test_move_into_group_undo_redo_focus_the_same_outer_tab() {
        use opossum_core::nodes::{Dummy, NodeGroup};

        let app_state = Data::new(AppState::default());
        let (root_id, node_a, inner_group_id) = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();
            let root_id = scenery.node_attr().uuid();
            let node_a = scenery.add_node(Dummy::default()).unwrap();
            let inner_group_id = scenery.add_node(NodeGroup::new("inner")).unwrap();
            (root_id, node_a, inner_group_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_move_nodes)
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/move_nodes")
            .set_json(&MoveNodesRequest {
                source_group_id: root_id,
                target_group_id: inner_group_id,
                nodes_to_move: vec![node_a],
            })
            .to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);

        // The pushed undo command (stacks are push_back/pop_back, so the top is `back`) must focus the
        // outer root tab, not the inner group.
        let undo_jump = {
            let stack = app_state.undo_stack.lock();
            stack.back().unwrap().jump_target(root_id).unwrap()
        };
        assert_eq!(
            undo_jump.graph_id, root_id,
            "undo of a move-into-group must focus the outer tab"
        );

        // Undo, then the re-inverted redo command must focus the *same* outer tab - not the inner group.
        let req = test::TestRequest::post().uri("/undo").to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);
        let redo_jump = {
            let stack = app_state.redo_stack.lock();
            stack.back().unwrap().jump_target(root_id).unwrap()
        };
        assert_eq!(
            redo_jump.graph_id, root_id,
            "redo of the same move must focus the same outer tab, not jump into the group"
        );
    }

    /// Regression test for the bug where converting nodes that already have an external port
    /// mapping into a new group silently lost that mapping - a genuine backend data bug, not just
    /// a display gap: `post_convert_nodes_to_group` never inspected the source group's own port
    /// map for the nodes being converted, and its node-deletion step unconditionally stripped any
    /// such mapping with nothing recreating it. Fixed by restructuring the endpoint to reuse
    /// `disconnect_moved_node_connections`/`reconnect_moved_node_connections` - the same reroute
    /// logic already proven correct for drag-and-drop moves (see
    /// `test_move_node_preserves_connections_via_auto_mapping`, which this mirrors). Builds
    /// `root { P { A, B } }` where `A`'s output is mapped to `P`'s external port `ext_out_1`,
    /// connected to a sibling `S` at root level (`P -> S`), converts `{A, B}` into a new group `Q`
    /// nested inside `P`, and asserts: both `Q` and `P` are reported as needing a port-handle
    /// refresh; `P`'s `ext_out_1` mapping now resolves through `Q` instead of directly to `A`; the
    /// root-level `P -> S` edge is completely untouched; a single undo restores `P`'s direct
    /// mapping to `A`; a following redo re-establishes the routed-through-`Q` state.
    #[actix_web::test]
    async fn test_convert_nodes_to_group_preserves_pre_existing_port_mapping() {
        use opossum_core::{
            nodes::{Dummy, NodeGroup},
            prelude::PortType,
        };

        let app_state = Data::new(AppState::default());
        let (p_id, node_a, node_b, sibling_s) = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();

            let mut p = NodeGroup::new("P");
            let node_a = p.add_node(Dummy::default()).unwrap();
            let node_b = p.add_node(Dummy::default()).unwrap();
            p.with_node_attr_mut(node_a, |attr| {
                attr.set_gui_position(Some(Point2::new(0.0, 0.0)));
            })
            .unwrap();
            p.with_node_attr_mut(node_b, |attr| {
                attr.set_gui_position(Some(Point2::new(50.0, 0.0)));
            })
            .unwrap();
            p.map_output_port(node_a, "output_1", "ext_out_1").unwrap();
            let p_id = scenery.add_node(p).unwrap();

            let sibling_s = scenery.add_node(Dummy::default()).unwrap();
            scenery
                .connect_nodes(p_id, "ext_out_1", sibling_s, "input_1", meter!(0.1))
                .unwrap();

            (p_id, node_a, node_b, sibling_s)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_convert_nodes_to_group)
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/convert_to_group")
            .set_json(&ConvertToGroupRequest {
                group_id: p_id,
                nodes_to_convert: vec![node_a, node_b],
            })
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let response: ConvertToGroupResponse = test::read_body_json(resp).await;
        let new_group_id = response.new_group.uuid();

        assert!(
            response.port_map_groups_changed.contains(&new_group_id)
                && response.port_map_groups_changed.contains(&p_id),
            "both the new group and P must be reported as needing a port-handle refresh"
        );

        let assert_routed_through_new_group = |app_state: &Data<AppState>| {
            let document = app_state.document.lock();
            let mapping = document
                .scenery()
                .with_group_node(p_id, |g| {
                    g.graph()
                        .port_map(&PortType::Output)
                        .get("ext_out_1")
                        .cloned()
                })
                .unwrap()
                .expect("P's ext_out_1 mapping must still exist");
            assert_eq!(
                mapping.0, new_group_id,
                "P's ext_out_1 mapping must resolve through the new group, not directly to A"
            );
            let inner_name = document
                .scenery()
                .with_group_node(new_group_id, |g| {
                    g.graph()
                        .port_map(&PortType::Output)
                        .external_port_of_mapped_port(node_a, "output_1")
                })
                .unwrap();
            assert!(
                inner_name.is_some(),
                "the new group must expose A's output under some external name"
            );
            let root_connections = document.scenery().graph().connections();
            assert!(
                root_connections
                    .iter()
                    .any(|c| c.src_id == p_id && c.target_id == sibling_s),
                "the root-level P -> S connection must be completely untouched"
            );
        };
        assert_routed_through_new_group(&app_state);

        let req = test::TestRequest::post().uri("/undo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "undo must not error"
        );
        {
            let document = app_state.document.lock();
            assert!(
                document.scenery().node_recursive(new_group_id).is_err(),
                "the new group must be gone after undo"
            );
            let mapping = document
                .scenery()
                .with_group_node(p_id, |g| {
                    g.graph()
                        .port_map(&PortType::Output)
                        .get("ext_out_1")
                        .cloned()
                })
                .unwrap();
            assert_eq!(
                mapping,
                Some((node_a, "output_1".to_string())),
                "undo must restore P's mapping directly to A"
            );
            let root_connections = document.scenery().graph().connections();
            assert!(
                root_connections
                    .iter()
                    .any(|c| c.src_id == p_id && c.target_id == sibling_s),
                "the root-level P -> S connection must still be untouched after undo"
            );
        }

        let req = test::TestRequest::post().uri("/redo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "redo must not error"
        );
        assert_routed_through_new_group(&app_state);
    }

    /// Regression test for the bug where converting a group's own mapped members into a new subgroup
    /// silently dropped the mapping instead of rerouting it, specifically when nothing is currently
    /// plugged into the group's exposed port. Root cause:
    /// `disconnect_moved_node_connections`'s handling of `find_pre_existing_mapping_consumer`'s
    /// `Orphaned` result unconditionally treated "nothing outward currently consumes this export" as
    /// "safe to drop" - correct for a real cross-group move (the member leaves for good, nothing is
    /// left for the old mapping to point at), but wrong for convert-to-group, where the source group
    /// never disappears - the member just relocates one level deeper inside it, so the source group's
    /// own export stays just as meaningful regardless of whether anything happens to be connected to
    /// it right now (this is exactly the state a freshly pasted/cut group is in, before being rewired
    /// to a neighbor). Same `root { P { A, B } }` setup as
    /// `test_convert_nodes_to_group_preserves_pre_existing_port_mapping`, but deliberately *without*
    /// the live `P -> S` connection that test wires up - so `P`'s `ext_out_1` mapping has no live
    /// consumer anywhere outward at all. Asserts the mapping still survives, rerouted through the new
    /// group, exactly as it does in the live-connected case.
    #[actix_web::test]
    async fn test_convert_nodes_to_group_preserves_unconnected_pre_existing_port_mapping() {
        use opossum_core::{
            nodes::{Dummy, NodeGroup},
            prelude::PortType,
        };

        let app_state = Data::new(AppState::default());
        let (p_id, node_a, node_b) = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();

            let mut p = NodeGroup::new("P");
            let node_a = p.add_node(Dummy::default()).unwrap();
            let node_b = p.add_node(Dummy::default()).unwrap();
            p.with_node_attr_mut(node_a, |attr| {
                attr.set_gui_position(Some(Point2::new(0.0, 0.0)));
            })
            .unwrap();
            p.with_node_attr_mut(node_b, |attr| {
                attr.set_gui_position(Some(Point2::new(50.0, 0.0)));
            })
            .unwrap();
            p.map_output_port(node_a, "output_1", "ext_out_1").unwrap();
            let p_id = scenery.add_node(p).unwrap();

            (p_id, node_a, node_b)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_convert_nodes_to_group),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/convert_to_group")
            .set_json(&ConvertToGroupRequest {
                group_id: p_id,
                nodes_to_convert: vec![node_a, node_b],
            })
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let response: ConvertToGroupResponse = test::read_body_json(resp).await;
        let new_group_id = response.new_group.uuid();

        let document = app_state.document.lock();
        let mapping = document
            .scenery()
            .with_group_node(p_id, |g| {
                g.graph()
                    .port_map(&PortType::Output)
                    .get("ext_out_1")
                    .cloned()
            })
            .unwrap()
            .expect(
                "P's ext_out_1 mapping must still exist even though nothing was plugged into it",
            );
        assert_eq!(
            mapping.0, new_group_id,
            "P's ext_out_1 mapping must resolve through the new group, not directly to A"
        );
        let inner_name = document
            .scenery()
            .with_group_node(new_group_id, |g| {
                g.graph()
                    .port_map(&PortType::Output)
                    .external_port_of_mapped_port(node_a, "output_1")
            })
            .unwrap();
        assert!(
            inner_name.is_some(),
            "the new group must expose A's output under some external name"
        );
    }

    /// Regression test for the bug where a *second*, deeper convert-to-group call lost a
    /// pre-existing mapping instead of rerouting it, because `disconnect_moved_node_connections`'s
    /// discovery of "what consumes this export" only ever checked exactly one hop up - correct for
    /// a first conversion (whose only possible consumer is a live edge or nothing, one hop out), but
    /// wrong once a prior conversion has already chained the mapping through an intermediate group.
    /// Builds `root { P { A, B } }` exactly as in
    /// `test_convert_nodes_to_group_preserves_pre_existing_port_mapping`, converts `{A, B}` into `Q`
    /// (so `P`'s `ext_out_1` now resolves through `Q`, not directly to `A` - the already-tested
    /// single-hop case), then goes one level deeper and converts `{A}` alone into a brand-new group
    /// `R` nested inside `Q`. Asserts: `R` exposes `A`'s output; `Q`'s own mapping now resolves
    /// through `R` instead of directly to `A`; `P`'s mapping and the root-level `P -> S` connection
    /// are byte-for-byte unchanged; `port_map_groups_changed` contains `Q` and `R` but explicitly not
    /// `P` (the concrete, checkable expression of "`P` is untouched"); undo restores `Q`'s direct
    /// mapping to `A` and removes `R` without disturbing `P` or the root connection; redo
    /// re-establishes the routed-through-`R` state.
    #[actix_web::test]
    async fn test_convert_nodes_to_group_reroutes_mapping_chained_through_two_levels() {
        use opossum_core::{
            nodes::{Dummy, NodeGroup},
            prelude::PortType,
        };

        let app_state = Data::new(AppState::default());
        let (p_id, node_a, node_b, sibling_s) = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();

            let mut p = NodeGroup::new("P");
            let node_a = p.add_node(Dummy::default()).unwrap();
            let node_b = p.add_node(Dummy::default()).unwrap();
            p.with_node_attr_mut(node_a, |attr| {
                attr.set_gui_position(Some(Point2::new(0.0, 0.0)));
            })
            .unwrap();
            p.with_node_attr_mut(node_b, |attr| {
                attr.set_gui_position(Some(Point2::new(50.0, 0.0)));
            })
            .unwrap();
            p.map_output_port(node_a, "output_1", "ext_out_1").unwrap();
            let p_id = scenery.add_node(p).unwrap();

            let sibling_s = scenery.add_node(Dummy::default()).unwrap();
            scenery
                .connect_nodes(p_id, "ext_out_1", sibling_s, "input_1", meter!(0.1))
                .unwrap();

            (p_id, node_a, node_b, sibling_s)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_convert_nodes_to_group)
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        // First conversion: {A, B} -> Q, nested inside P. Already covered on its own by
        // `test_convert_nodes_to_group_preserves_pre_existing_port_mapping`; repeated here only to
        // set up the mapping chain the second conversion needs to reroute through.
        let req = test::TestRequest::post()
            .uri("/convert_to_group")
            .set_json(&ConvertToGroupRequest {
                group_id: p_id,
                nodes_to_convert: vec![node_a, node_b],
            })
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let response: ConvertToGroupResponse = test::read_body_json(resp).await;
        let q_id = response.new_group.uuid();

        let q_inner_name = {
            let document = app_state.document.lock();
            document
                .scenery()
                .with_group_node(q_id, |g| {
                    g.graph()
                        .port_map(&PortType::Output)
                        .external_port_of_mapped_port(node_a, "output_1")
                })
                .unwrap()
                .expect("Q must expose A's output under some external name")
        };

        // Second conversion, one level deeper: convert {A} alone - now living directly in Q, with
        // Q's own mapping (not a live edge) the thing consuming its export - into a brand-new group
        // R nested inside Q.
        let req = test::TestRequest::post()
            .uri("/convert_to_group")
            .set_json(&ConvertToGroupRequest {
                group_id: q_id,
                nodes_to_convert: vec![node_a],
            })
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let response: ConvertToGroupResponse = test::read_body_json(resp).await;
        let r_id = response.new_group.uuid();

        assert!(
            response.port_map_groups_changed.contains(&q_id)
                && response.port_map_groups_changed.contains(&r_id),
            "both Q and the new group R must be reported as needing a port-handle refresh"
        );
        assert!(
            !response.port_map_groups_changed.contains(&p_id),
            "P's own mapping must be completely untouched by a conversion two levels below it"
        );

        let assert_routed_through_r = |app_state: &Data<AppState>| {
            let document = app_state.document.lock();

            let q_mapping = document
                .scenery()
                .with_group_node(q_id, |g| {
                    g.graph()
                        .port_map(&PortType::Output)
                        .get(&q_inner_name)
                        .cloned()
                })
                .unwrap()
                .expect("Q's own mapping entry must still exist");
            assert_eq!(
                q_mapping.0, r_id,
                "Q's mapping must resolve through the new group R, not directly to A"
            );

            let r_inner_name = document
                .scenery()
                .with_group_node(r_id, |g| {
                    g.graph()
                        .port_map(&PortType::Output)
                        .external_port_of_mapped_port(node_a, "output_1")
                })
                .unwrap();
            assert!(
                r_inner_name.is_some(),
                "R must expose A's output under some external name"
            );

            let p_mapping = document
                .scenery()
                .with_group_node(p_id, |g| {
                    g.graph()
                        .port_map(&PortType::Output)
                        .get("ext_out_1")
                        .cloned()
                })
                .unwrap();
            assert_eq!(
                p_mapping,
                Some((q_id, q_inner_name.clone())),
                "P's own mapping must be completely unchanged - still resolving through Q exactly \
                 as the first conversion left it"
            );

            let root_connections = document.scenery().graph().connections();
            assert!(
                root_connections
                    .iter()
                    .any(|c| c.src_id == p_id && c.target_id == sibling_s),
                "the root-level P -> S connection must remain completely untouched"
            );
        };
        assert_routed_through_r(&app_state);

        let req = test::TestRequest::post().uri("/undo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "undo must not error"
        );
        {
            let document = app_state.document.lock();
            assert!(
                document.scenery().node_recursive(r_id).is_err(),
                "R must be gone after undo"
            );
            let q_mapping = document
                .scenery()
                .with_group_node(q_id, |g| {
                    g.graph()
                        .port_map(&PortType::Output)
                        .get(&q_inner_name)
                        .cloned()
                })
                .unwrap();
            assert_eq!(
                q_mapping,
                Some((node_a, "output_1".to_string())),
                "undo must restore Q's mapping directly to A"
            );
            let p_mapping = document
                .scenery()
                .with_group_node(p_id, |g| {
                    g.graph()
                        .port_map(&PortType::Output)
                        .get("ext_out_1")
                        .cloned()
                })
                .unwrap();
            assert_eq!(
                p_mapping,
                Some((q_id, q_inner_name.clone())),
                "P's mapping must still be untouched after undoing the second conversion"
            );
            let root_connections = document.scenery().graph().connections();
            assert!(
                root_connections
                    .iter()
                    .any(|c| c.src_id == p_id && c.target_id == sibling_s),
                "the root-level P -> S connection must still be untouched after undo"
            );
        }

        let req = test::TestRequest::post().uri("/redo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "redo must not error"
        );
        assert_routed_through_r(&app_state);
    }
}
