use std::collections::{HashMap, HashSet};

use actix_web::{
    post,
    web::{self, Json},
};
use nalgebra::Point2;
use opossum_core::{
    core_optics::{NodeAttrExt, OpticRef, node_attr::HasNodeAttr},
    error::OpossumError,
    nodes::{ConnectionInfo, NodeGroup, NodeReference, create_node_ref},
    opm_document::{AnalyzerInfo, OpmDocument},
    prelude::{OpticNode, PortMap, PortType, Proptype},
    types::api_types::{AnalyzerItemDto, ConnectInfo, ErrorResponse, NodeInfo, PasteNodesResponse},
    utils::LockExt,
};
use uuid::Uuid;

use super::upper_left_corner_of_nodes;
use crate::{
    app_state::{AppState, NodeCacheItem},
    error::BackEndErrorResponse,
    helper_functions::{
        build_connect_info, capture_node_connections, map_port, parent_group_id_or_self,
        validate_relocated_references,
    },
    undo::{CascadedNode, Command, EdgeSnapshot, NodeSnapshot},
};

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

/// Splits `items` into its optical and analyzer members, preserving relative order within each.
fn partition_cache(
    items: impl IntoIterator<Item = NodeCacheItem>,
) -> (Vec<OpticRef>, Vec<AnalyzerItemDto>) {
    let mut optical = Vec::new();
    let mut analyzer = Vec::new();
    for item in items {
        match item {
            NodeCacheItem::Optical(o) => optical.push(o),
            NodeCacheItem::Analyzer(a) => analyzer.push(a),
        }
    }
    (optical, analyzer)
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

/// Finds, among `top_level_ids`, which ones are `NodeReference`s pointing at *another* member of
/// `top_level_ids`, and returns them grouped by their target's uuid as [`CascadedNode`]s.
///
/// Mirrors `nodes/core.rs`'s `capture_cascade`: `NodeGroup::delete_node` cascades a target's removal to
/// every reference node anywhere in the document that points at it (recursively, across nested groups -
/// see `OpticGraph::delete_node`'s doc comment), so a reference pasted alongside its own target must not
/// also get its own independent `RemoveNode` in the same batch - by the time that ran, the target's own
/// `RemoveNode` may already have cascaded the reference away, 400ing with "node with given uuid does not
/// exist" (order-dependent on `grouped_node_infos`' iteration order, so it doesn't always reproduce).
/// Folding it into the target's own `NodeSnapshot.cascaded` instead makes the target's `RemoveNode`/
/// `AddNode` pair remove/restore both together, order-independently. Only *top-level* siblings are
/// considered - a reference nested inside a pasted group pointing at a top-level sibling is a rarer,
/// differently-shaped edge case (silent data loss via shared-`Arc` semantics, not a 400) that this pass
/// doesn't cover.
fn cascaded_references(
    document: &OpmDocument,
    paste_group_id: Uuid,
    top_level_ids: &HashSet<Uuid>,
) -> HashMap<Uuid, Vec<CascadedNode>> {
    let mut by_target = HashMap::<Uuid, Vec<CascadedNode>>::new();
    let root_id = document.scenery().node_attr().uuid();
    for target_id in top_level_ids {
        let Ok(referring) = document
            .scenery()
            .graph()
            .find_all_nodes_referring_to_uuid(*target_id, root_id)
        else {
            continue;
        };
        for ref_id in referring.values().flatten() {
            if ref_id == target_id || !top_level_ids.contains(ref_id) {
                continue;
            }
            if let Ok((node, _)) = document.scenery().node_recursive(*ref_id) {
                by_target.entry(*target_id).or_default().push(CascadedNode {
                    parent_group_id: paste_group_id,
                    node,
                    // A top-level pasted node's own wiring is already fully captured by the
                    // mutual-`RemoveEdge`/`AddEdge` mechanism above, so it must not be captured
                    // again here too.
                    connections: Vec::new(),
                });
            }
        }
    }
    by_target
}

/// Builds the "undo the whole paste" batch for [`post_paste_nodes`]: one `RemoveNode` per *top-level*
/// pasted node, one `RemoveAnalyzer` per pasted analyzer, plus a leading `RemoveEdge` for every mutual
/// connection between two top-level pasted nodes.
///
/// Only the *top-level* pasted roots need their own `RemoveNode` - a group's own `OpticRef` already
/// carries its entire internal subtree (nodes and internal edges) as one live object, so removing it via
/// a single `RemoveNode` already correctly captures/restores everything inside it. Giving a nested
/// descendant (an entry under any *other* key of `grouped_node_infos` - a freshly-created nested group's
/// own uuid) its own separate `RemoveNode` too is not just redundant but harmful: `Command::Batch`
/// applies its commands in `Vec` order, itself derived from this map's non-deterministic iteration
/// order, so a nested entry can end up targeting a uuid its own ancestor's `RemoveNode` already cascaded
/// away (surfacing as "node with given uuid does not exist" on undo), or mutate the group's live
/// internal graph directly *before* the group's own command runs, silently severing an internal
/// connection that nothing later restores (a nested `AddNode` always passes an empty `connections` list,
/// so redo can't reconnect what this already tore down). The same is true of a top-level pasted
/// `NodeReference` targeting another top-level pasted node - see [`cascaded_references`] - so those are
/// folded into their target's own `RemoveNode` instead of getting one of their own.
///
/// A freshly pasted node's only connections are to other nodes pasted in the same gesture
/// (`insert_copied_nodes` only ever recreates connections between copied nodes) - so every connection
/// touching a top-level pasted node is "mutual" in `capture_and_split_mutual_connections`'s sense.
/// Restore each one once via a *leading* `RemoveEdge`, positioned before the `RemoveNode`s: on the first
/// undo this disconnects the pair while both nodes still exist, and thanks to `Command::Batch` reversing
/// its inverses, the resulting redo batch adds both nodes back before restoring the edge - never the
/// other way around, which would try to reconnect to a node redo hasn't re-added yet.
fn build_paste_undo_batch(
    document: &OpmDocument,
    paste_group_id: Uuid,
    grouped_node_infos: &HashMap<Uuid, Vec<NodeInfo>>,
    analyzers: &[AnalyzerItemDto],
) -> Vec<Command> {
    let mut removals = Vec::new();
    if let Some(infos) = grouped_node_infos.get(&paste_group_id) {
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
        let mut cascaded = cascaded_references(document, paste_group_id, &top_level_ids);
        let folded_ids: HashSet<Uuid> = cascaded
            .values()
            .flatten()
            .filter_map(|c| c.node.uuid().ok())
            .collect();
        for info in infos {
            if folded_ids.contains(&info.uuid()) {
                continue;
            }
            if let Ok((node_ref, _)) = document.scenery().node_recursive(info.uuid()) {
                removals.push(Command::RemoveNode(NodeSnapshot {
                    parent_group_id: paste_group_id,
                    node: node_ref,
                    cascaded: cascaded.remove(&info.uuid()).unwrap_or_default(),
                    connections: Vec::new(),
                }));
            }
        }
    }
    for analyzer in analyzers {
        removals.push(Command::RemoveAnalyzer(analyzer.clone()));
    }
    removals
}

/// Paste copied nodes
///
/// This function duplicates the nodes/analyzers currently in the copy cache into the target group,
/// minting a fresh uuid for each copy. Moving nodes without duplicating them (a "cut") is a separate
/// operation - see [`post_cut_nodes`](super::cut::post_cut_nodes).
///
/// Rejected (before anything is inserted, so the copy cache is left intact for a retry elsewhere) if a
/// copied `NodeReference` - uncopied targets aren't duplicated, so it would still resolve to the same live
/// target - would end up nested inside its own target group, or a group nested within it; see
/// [`validate_relocated_references`].
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
pub(super) async fn post_paste_nodes(
    data: web::Data<AppState>,
    node_paste_info: web::Json<(Uuid, (f64, f64))>,
) -> Result<Json<PasteNodesResponse>, BackEndErrorResponse> {
    let (paste_group_id, node_pos) = node_paste_info.into_inner();
    let paste_in_scenery = data.document.lock().scenery().node_attr().uuid() == paste_group_id;

    let copied_nodes = data.node_copy_cache.lock();
    let min_pos = upper_left_corner_of_nodes(&copied_nodes)?;
    drop(copied_nodes);
    let shift = Point2::new(node_pos.0 - min_pos.x, node_pos.1 - min_pos.y);

    let (copied_optical_nodes, copied_analyzer_nodes) =
        partition_cache(data.node_copy_cache.lock().iter().cloned());

    let mut analyzers = Vec::new();
    if paste_in_scenery {
        for analyzer_dto in &copied_analyzer_nodes {
            // Pass the internal AnalyzerInfo to copy_analyzer
            analyzers.push(copy_analyzer(&data, shift, &analyzer_dto.info));
        }
    }

    let mut document = data.document.lock();
    let root_ids: Vec<Uuid> = copied_optical_nodes
        .iter()
        .filter_map(|r| r.uuid().ok())
        .collect();
    validate_relocated_references(document.scenery(), &root_ids, paste_group_id)?;

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
    // See `build_paste_undo_batch` for why only top-level pasted roots get their own `RemoveNode`.
    let removals =
        build_paste_undo_batch(&document, paste_group_id, &grouped_node_infos, &analyzers);
    if !removals.is_empty() {
        data.push_undo(Command::Batch(removals));
    }

    Ok(Json(PasteNodesResponse {
        pasted_nodes: grouped_node_infos,
        pasted_analyzers: analyzers,
        pasted_connections: grouped_connect_info,
    }))
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
    // output port maps, then input port maps
    for port_type in [PortType::Output, PortType::Input] {
        let port_maps = match port_type {
            PortType::Output => output_port_maps,
            PortType::Input => input_port_maps,
        };
        for (old_group_id, _, _) in grouped_node_refs {
            let Some(port_map) = port_maps.get(old_group_id) else {
                continue;
            };
            for (external_port_name, (input_node, internal_port_name)) in port_map {
                if let (Some(new_group_id), Some(new_mapped_node_id)) =
                    (node_id_link.get(old_group_id), node_id_link.get(input_node))
                {
                    scenery.with_group_node_mut(*new_group_id, |new_group| {
                        map_port(
                            new_group,
                            port_type,
                            *new_mapped_node_id,
                            internal_port_name,
                            external_port_name,
                        )?;
                        Ok::<(), BackEndErrorResponse>(())
                    })??;
                }
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
fn collect_optical_nodes_to_copy_recursive(
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
                let info = build_connect_info(
                    scenery,
                    c.src_id,
                    &c.src_port,
                    c.target_id,
                    &c.target_port,
                    c.distance.value,
                );
                (c, info)
            })
            .collect();

        scenery
            .with_group_node_mut(group_id, |group| -> Result<(), BackEndErrorResponse> {
                for (c, info) in enriched {
                    group.connect_nodes(
                        c.src_id,
                        &c.src_port,
                        c.target_id,
                        &c.target_port,
                        c.distance,
                    )?;

                    result.push(info);
                }
                Ok(())
            })
            .map_err(|e| {
                BackEndErrorResponse::new(404, "Opossum", &format!("Could not paste nodes: {e}"))
            })??;
    }

    Ok(result)
}

fn collect_optical_node_to_copy(
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

fn copy_from_optic_ref(
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

#[cfg(test)]
mod test {
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use opossum_core::meter;

    use super::*;
    use crate::{
        document::{redo_document, undo_document},
        operations::copy::post_copy_nodes,
    };

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

    /// Regression test for the bug where copying a node *and* a reference to it together, pasting them,
    /// then undoing 400'd with "node with given uuid does not exist". Root cause: `build_paste_undo_batch`
    /// gave both the pasted node and the pasted reference their own independent top-level `RemoveNode`;
    /// removing the node cascades (via `NodeGroup::delete_node`) to also delete the reference pointing at
    /// it, so the reference's own separate `RemoveNode` then found nothing to delete. Builds `A` and
    /// `R = NodeReference::from_node(&A)` as flat top-level nodes, copies and pastes both together, and
    /// asserts undo succeeds (removing both pasted duplicates) and a following redo restores both, with
    /// the pasted reference resolving to the pasted node's uuid (not the original `A`'s).
    #[actix_web::test]
    async fn test_undo_redo_paste_node_with_reference_same_group() {
        use opossum_core::nodes::Dummy;

        let app_state = Data::new(AppState::default());
        let (root_id, node_a) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();
            let node_a = scenery.add_node(Dummy::default()).unwrap();
            let node_a_ref = scenery.node_recursive(node_a).unwrap().0;
            let node_reference = NodeReference::from_node(&node_a_ref).unwrap();
            scenery.add_node(node_reference).unwrap();
            (root_id, node_a)
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
        nodes_to_copy.insert(
            app_state
                .document
                .lock()
                .scenery()
                .with_group_node(root_id, |g| {
                    g.nodes()
                        .iter()
                        .filter_map(|n| n.uuid().ok())
                        .find(|id| *id != node_a)
                })
                .unwrap()
                .expect("the reference node must exist"),
        );
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

        let pasted_ids: Vec<Uuid> = {
            let document = app_state.document.lock();
            document
                .scenery()
                .with_group_node(root_id, |g| {
                    g.nodes()
                        .iter()
                        .filter_map(|n| n.uuid().ok())
                        .filter(|id| !nodes_to_copy.contains(id))
                        .collect::<Vec<_>>()
                })
                .unwrap()
        };
        assert_eq!(
            pasted_ids.len(),
            2,
            "both the pasted node and its pasted reference must exist"
        );

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "undoing the paste of a node with a reference to it must not error"
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
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "redo must not error");

        let document = app_state.document.lock();
        for id in &pasted_ids {
            assert!(
                document.scenery().node_recursive(*id).is_ok(),
                "the pasted duplicate must be restored by redo"
            );
        }
        // Identify which of the two pasted nodes is the reference by its node type, then assert its
        // "reference id" points at the *other* pasted node, not at the original `A`.
        let (pasted_ref_id, pasted_target_id) = {
            let mut ref_id = None;
            let mut target_id = None;
            for id in &pasted_ids {
                let node_type = document
                    .scenery()
                    .with_node_attr(*id, |attr| attr.node_type().to_string())
                    .unwrap();
                if node_type == "reference" {
                    ref_id = Some(*id);
                } else {
                    target_id = Some(*id);
                }
            }
            (
                ref_id.expect("a pasted reference node must exist"),
                target_id.expect("a pasted target node must exist"),
            )
        };
        let ref_target = document
            .scenery()
            .with_node_attr(pasted_ref_id, |attr| {
                attr.properties().get("reference id").cloned()
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            ref_target,
            Proptype::Uuid(pasted_target_id),
            "the pasted reference must resolve to the pasted node's uuid, not the original A's"
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

    /// Regression test for the bug where nothing stopped a reference to a group from being pasted into
    /// that very group - which deadlocks the analyzer (analyzing a group holds its `Mutex` for the
    /// duration of its own recursive descent, so a reference resolving back to an already-locked ancestor
    /// self-deadlocks). Copies `R = ref(G)` alone (leaving `G` uncopied, so the paste would still resolve
    /// to the same live `G`) and asserts pasting it into `G` is rejected, with nothing inserted.
    #[actix_web::test]
    async fn test_paste_reference_into_own_target_is_rejected() {
        use opossum_core::nodes::NodeGroup;

        let app_state = Data::new(AppState::default());
        let (g_id, ref_id) = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();
            let g_id = scenery.add_node(NodeGroup::new("G")).unwrap();
            let g_ref = scenery.node(g_id).unwrap();
            let node_reference = NodeReference::from_node(&g_ref).unwrap();
            let ref_id = scenery.add_node(node_reference).unwrap();
            (g_id, ref_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_copy_nodes)
                .service(post_paste_nodes),
        )
        .await;

        let mut nodes_to_copy = HashSet::new();
        nodes_to_copy.insert(ref_id);
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
            .set_json(&(g_id, (0.0, 0.0)))
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::BAD_REQUEST,
            "pasting a reference into its own target group must be rejected"
        );
        assert_eq!(
            app_state
                .document
                .lock()
                .scenery()
                .with_group_node(g_id, NodeGroup::nr_of_nodes)
                .unwrap(),
            0,
            "nothing must have been inserted into the target group"
        );
    }

    /// Same hazard, one level deeper: `G1` contains `G2`; a reference to `G1` sitting at the root must
    /// also be rejected when pasted into `G2`, since `G2` lives inside `G1`'s own subtree too.
    #[actix_web::test]
    async fn test_paste_reference_into_nested_descendant_of_target_is_rejected() {
        use opossum_core::nodes::{Dummy, NodeGroup};

        let app_state = Data::new(AppState::default());
        let (g2_id, ref_id) = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();
            let mut g2 = NodeGroup::new("G2");
            g2.add_node(Dummy::default()).unwrap();
            let mut g1 = NodeGroup::new("G1");
            let g2_id = g1.add_node(g2).unwrap();
            let g1_id = scenery.add_node(g1).unwrap();
            let g1_ref = scenery.node(g1_id).unwrap();
            let node_reference = NodeReference::from_node(&g1_ref).unwrap();
            let ref_id = scenery.add_node(node_reference).unwrap();
            (g2_id, ref_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_copy_nodes)
                .service(post_paste_nodes),
        )
        .await;

        let mut nodes_to_copy = HashSet::new();
        nodes_to_copy.insert(ref_id);
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
            .set_json(&(g2_id, (0.0, 0.0)))
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::BAD_REQUEST,
            "pasting a reference into a nested descendant of its target must be rejected"
        );
    }

    /// A reference and its own target copied and pasted together, as siblings, into an unrelated
    /// destination group must still succeed - they keep the same (valid) relative structure either way,
    /// unlike the two rejected cases above.
    #[actix_web::test]
    async fn test_paste_reference_and_target_together_as_siblings_is_allowed() {
        use opossum_core::nodes::NodeGroup;

        let app_state = Data::new(AppState::default());
        let (g_id, ref_id, dest_id) = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();
            let g_id = scenery.add_node(NodeGroup::new("G")).unwrap();
            let g_ref = scenery.node(g_id).unwrap();
            let node_reference = NodeReference::from_node(&g_ref).unwrap();
            let ref_id = scenery.add_node(node_reference).unwrap();
            let dest_id = scenery.add_node(NodeGroup::new("dest")).unwrap();
            (g_id, ref_id, dest_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_copy_nodes)
                .service(post_paste_nodes),
        )
        .await;

        let mut nodes_to_copy = HashSet::new();
        nodes_to_copy.insert(g_id);
        nodes_to_copy.insert(ref_id);
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
            .set_json(&(dest_id, (0.0, 0.0)))
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "pasting a reference together with its own target as siblings must still be allowed"
        );
    }
}
