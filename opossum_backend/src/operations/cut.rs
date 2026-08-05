use std::collections::{HashMap, HashSet};

use actix_web::{
    post,
    web::{self, Json},
};
use nalgebra::Point2;
use opossum_core::{
    core_optics::node_attr::HasNodeAttr,
    opm_document::OpmDocument,
    prelude::PortType,
    types::api_types::{
        ConnectInfo, CutNodesResponse, ErrorResponse, MoveNodesRequest, NodeInfo, PositionUpdate,
        RelocatedNode,
    },
    utils::LockExt,
};
use uuid::Uuid;

use super::upper_left_corner_of_nodes;
use crate::{
    app_state::{AppState, NodeCacheItem},
    document::apply_position_updates,
    error::BackEndErrorResponse,
    helper_functions::{
        lowest_common_ancestor_group, relocate_nodes_severing_external_links, sever_external_links,
        split_cascades_for_response,
    },
    undo::{Command, EdgeSnapshot, MoveNodes},
};

/// Resolves each of `optical_ids`' current parent group in `document`, keyed by that group regardless
/// of whether it's the cut's target group - severing a cut node's links to anything left behind applies
/// uniformly whether or not the node ends up relocating. An id that's vanished since it was copied is
/// silently skipped.
fn group_cut_nodes_by_parent(
    document: &OpmDocument,
    optical_ids: &[Uuid],
) -> HashMap<Uuid, Vec<Uuid>> {
    let mut groups_by_parent = HashMap::<Uuid, Vec<Uuid>>::new();
    for id in optical_ids {
        if let Ok((_, parent)) = document.scenery().node_recursive(*id) {
            groups_by_parent.entry(parent).or_default().push(*id);
        }
    }
    groups_by_parent
}

/// Aggregated side effects of [`sever_or_relocate_sources`], across every source group a cut touched.
struct SeverOrRelocateOutcome {
    /// One inverse [`Command::MoveNodes`] per relocated (cross-group) source, moving those nodes back.
    move_inverses: Vec<Command>,
    /// [`Command::AddEdge`]/port-map-cascade inverses restoring every torn-down link.
    restore_commands: Vec<Command>,
    /// `(node_id, source_group_id)` for every node that was relocated (i.e. wasn't already in the
    /// target group).
    relocated_pairs: Vec<(Uuid, Uuid)>,
    removed_connections: Vec<(Uuid, ConnectInfo)>,
    port_map_groups_changed: Vec<Uuid>,
    removed_port_mappings: Vec<(Uuid, Uuid, String, PortType)>,
}

/// Sorts `groups_by_parent`'s keys for determinism (`HashMap` iteration order isn't stable), then for
/// each source group: if it's already `target_group_id`, severs its cut nodes' links to anything left
/// behind in place ([`sever_external_links`]); otherwise relocates them into `target_group_id`,
/// severing external links the same way ([`relocate_nodes_severing_external_links`]). Aggregates the
/// GUI-facing side effects and one inverse `MoveNodes` per relocated source group.
///
/// # Errors
///
/// Returns an error if a sever/relocate step fails (e.g. an attempt to move a group into its own
/// descendant, which leaves the destination unreachable once the group is detached from its source).
fn sever_or_relocate_sources(
    document: &mut OpmDocument,
    target_group_id: Uuid,
    mut groups_by_parent: HashMap<Uuid, Vec<Uuid>>,
) -> Result<SeverOrRelocateOutcome, BackEndErrorResponse> {
    let mut move_inverses = Vec::<Command>::new();
    let mut restore_commands = Vec::<Command>::new();
    let mut relocated_pairs = Vec::<(Uuid, Uuid)>::new();
    let mut removed_connections = Vec::<(Uuid, ConnectInfo)>::new();
    let mut port_map_groups_changed = Vec::<Uuid>::new();
    let mut removed_port_mappings = Vec::<(Uuid, Uuid, String, PortType)>::new();
    let mut sources: Vec<Uuid> = groups_by_parent.keys().copied().collect();
    sources.sort();
    for source in sources {
        let Some(ids) = groups_by_parent.remove(&source) else {
            continue;
        };
        if source == target_group_id {
            // Already in the target group: no relocation, just sever links to nodes outside the cut set
            // (including any port-map chain the node itself exposes) - the common "cut and paste in the
            // same scenery" case, minus the bug of keeping links to uncut siblings.
            let outcome = sever_external_links(document, source, &ids)?;
            let (cascade_connections, cascade_port_mappings) =
                split_cascades_for_response(&outcome.cascades);

            restore_commands.extend(outcome.removed_connections.iter().map(|(group_id, c)| {
                Command::AddEdge(EdgeSnapshot {
                    group_id: *group_id,
                    connect_info: c.clone(),
                })
            }));
            restore_commands.extend(outcome.cascades.iter().map(Command::from));

            removed_connections.extend(outcome.removed_connections);
            removed_connections.extend(cascade_connections);
            removed_port_mappings.extend(cascade_port_mappings);
            port_map_groups_changed.extend(outcome.port_map_groups_changed);
            continue;
        }
        // A cut relocates each node preserving its uuid, severing the connections and port mappings it
        // carried to anything left behind (rather than rerouting them, as a drag-and-drop move does), but
        // keeping links to other nodes cut in the same gesture.
        let outcome =
            relocate_nodes_severing_external_links(document, source, target_group_id, &ids)?;
        let (cascade_connections, cascade_port_mappings) =
            split_cascades_for_response(&outcome.cascades);

        // Undo restores the torn-down links, but only once the nodes are back in `source` (see the undo
        // batch assembly in `post_cut_nodes`): re-add every direct edge, then replay each port-map
        // cascade (one innermost-first `AddPortMap` chain + terminal `AddEdge` per cascade).
        restore_commands.extend(outcome.removed_connections.iter().map(|(group_id, c)| {
            Command::AddEdge(EdgeSnapshot {
                group_id: *group_id,
                connect_info: c.clone(),
            })
        }));
        restore_commands.extend(outcome.cascades.iter().map(Command::from));

        removed_connections.extend(outcome.removed_connections);
        removed_connections.extend(cascade_connections);
        removed_port_mappings.extend(cascade_port_mappings);

        let focus_group_id =
            lowest_common_ancestor_group(document.scenery(), source, target_group_id)?;
        let mut affected = outcome.port_map_groups_changed;
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

    Ok(SeverOrRelocateOutcome {
        move_inverses,
        restore_commands,
        relocated_pairs,
        removed_connections,
        port_map_groups_changed,
        removed_port_mappings,
    })
}

/// Repositions every cut node (relocated or same-group) - and every cut analyzer, when cutting into the
/// scenery root - to its shifted position. `PositionUpdate`s carry *absolute* positions, so `shift` is
/// added to each node's current position (a relocation preserves `gui_position`, so it's still the
/// original). Returns the reposition inverses (for the undo batch) and the subset of updates the GUI
/// must apply itself - `relocated_nodes` already carries the new position for anything that also moved
/// groups, via `relocated_set`.
///
/// # Errors
///
/// Returns an error if applying a position update fails.
fn reposition_cut_nodes(
    document: &mut OpmDocument,
    target_group_id: Uuid,
    scenery_root: Uuid,
    optical_ids: &[Uuid],
    analyzer_ids: &[Uuid],
    shift: Point2<f64>,
    relocated_set: &HashSet<Uuid>,
) -> Result<(Vec<PositionUpdate>, Vec<Command>), BackEndErrorResponse> {
    let mut position_updates = Vec::<PositionUpdate>::new();
    for id in optical_ids {
        if let Ok(maybe_pos) = document
            .scenery()
            .with_node_attr(*id, opossum_core::core_optics::NodeAttr::gui_position)
        {
            let (cx, cy) = maybe_pos.map_or((0.0, 0.0), |p| (p.x, p.y));
            position_updates.push(PositionUpdate {
                uuid: *id,
                is_optical: true,
                gui_position: (cx + shift.x, cy + shift.y),
            });
        }
    }
    if target_group_id == scenery_root {
        for id in analyzer_ids {
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
    let position_inverses = apply_position_updates(document, position_updates)?;
    Ok((repositioned, position_inverses))
}

/// Captures each relocated node's final `NodeInfo` (post-reposition) for the target tab.
///
/// # Errors
///
/// Returns an error if locking a relocated node's optical ref fails.
fn build_relocated_node_infos(
    document: &OpmDocument,
    target_group_id: Uuid,
    relocated_pairs: Vec<(Uuid, Uuid)>,
) -> Result<Vec<RelocatedNode>, BackEndErrorResponse> {
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
    Ok(relocated_nodes)
}

/// Cut the copy cache into a group as a UUID-preserving *move* that severs the cut nodes' links to
/// anything left behind, while keeping links between two nodes that were cut together.
///
/// A cut relocates the *same* nodes rather than duplicating them (as
/// [`post_paste_nodes`](super::paste::post_paste_nodes) does): each keeps its uuid, so a
/// [`NodeReference`](opossum_core::prelude::NodeReference) pointing at it stays valid with no remapping
/// (this is why the cut is a move and not a copy - it's the fix for the undo/redo-of-references bug the
/// old duplicate cut had). Every connection and port mapping a cut node had to a node *not* in the cut -
/// whether that node lives in the same group or a different one - is severed ([`sever_external_links`]/
/// [`relocate_nodes_severing_external_links`]), exactly as the old duplicate cut left the pasted copy
/// after `delete_node`ing the original; connections between two co-cut nodes are preserved instead,
/// unlike a drag-and-drop move
/// ([`relocate_nodes_in_document`](crate::helper_functions::relocate_nodes_in_document)), which reroutes
/// rather than severs boundary connections. A cut node already in `target_group_id` is only
/// repositioned, not relocated (the common "cut and paste in the same scenery" case), but still has its
/// external links severed the same way. Analyzers can only ever be repositioned at the scenery root. The
/// whole gesture is one undo step ([`Command::Batch`]): the relocations' inverse [`Command::MoveNodes`]
/// (move the nodes back) first, then the link restores ([`Command::AddEdge`] / [`Command::AddPortMap`],
/// which reference the nodes back in their source group), then the reposition inverses - so undo/redo
/// revert it in one go, and because no uuid ever changes, redo can never hit the reference-cascade that a
/// duplicate cut did.
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
pub(super) async fn post_cut_nodes(
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

    let groups_by_parent = group_cut_nodes_by_parent(&document, &optical_ids);

    let SeverOrRelocateOutcome {
        move_inverses,
        mut restore_commands,
        relocated_pairs,
        removed_connections,
        port_map_groups_changed,
        removed_port_mappings,
    } = sever_or_relocate_sources(&mut document, target_group_id, groups_by_parent)?;
    let relocated_set: HashSet<Uuid> = relocated_pairs.iter().map(|(id, _)| *id).collect();

    let (repositioned, mut position_inverses) = reposition_cut_nodes(
        &mut document,
        target_group_id,
        scenery_root,
        &optical_ids,
        &analyzer_ids,
        shift,
        &relocated_set,
    )?;

    // Now that positions are final, capture each relocated node's `NodeInfo` for the target tab.
    let relocated_nodes = build_relocated_node_infos(&document, target_group_id, relocated_pairs)?;

    // One undo step for the whole gesture, ordered so each step's prerequisites already exist when it
    // runs (`Command::Batch` applies in order): first the relocate-back moves (every cut node returns to
    // its source group), then the link restores (`AddEdge` / `AddPortMap`, which reference those nodes in
    // their source group), then the reposition-back patches. Redo replays the reversed order - links torn
    // down again before the nodes move back out.
    let mut undo_batch = move_inverses;
    undo_batch.append(&mut restore_commands);
    undo_batch.append(&mut position_inverses);
    if !undo_batch.is_empty() {
        data.push_undo(Command::Batch(undo_batch));
    }

    drop(document);
    Ok(Json(CutNodesResponse {
        relocated_nodes,
        repositioned,
        // A cut severs boundary links rather than rerouting them, and any internal link it preserves
        // already existed - either untouched (same-group) or recreated silently and picked up by the
        // relocated tab's full refill (cross-group) - so nothing needs reporting as newly connected here.
        new_connections: Vec::new(),
        removed_connections,
        port_map_groups_changed,
        removed_port_mappings,
    }))
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use opossum_core::{meter, nodes::NodeReference, prelude::Proptype};
    use uuid::Uuid;

    use super::*;
    use crate::{
        app_state::AppState,
        document::{redo_document, undo_document},
        operations::copy::post_copy_nodes,
    };

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

    /// Regression test for the cut-of-a-connected-node bug: build `A -> G(B -> C) -> D` (the middle two
    /// grouped, so `G` exposes `B.input_1` as `g_in` and `C.output_1` as `g_out`, with `A -> G.g_in` and
    /// `G.g_out -> D`), plus a `NodeReference -> B`. Cutting `B` out of `G` into the root used to route
    /// through the move-preserve machinery, which 400'd ("source node with given id does not exist") on
    /// this nested case and left the graph half-mutated - `B` moved out but `G`'s `g_in` mapping and the
    /// `A -> G` edge dangling, so the stale mapping could no longer be deleted. The fix makes a cut
    /// cascade-delete the node's links (keeping its uuid, so the reference survives): after the cut `B`
    /// lives bare at the root, `G`'s `g_in` mapping and the `A -> G` edge are gone, and a single
    /// undo/redo round-trips the whole teardown.
    #[actix_web::test]
    async fn test_cut_connected_node_out_of_group_cascade_deletes_links() {
        use opossum_core::{
            meter,
            nodes::{Dummy, NodeGroup},
            prelude::PortType,
        };

        let app_state = Data::new(AppState::default());
        let (root_id, group_id, node_a, node_b, node_c, ref_id) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();

            let node_a = scenery.add_node(Dummy::default()).unwrap();
            let node_d = scenery.add_node(Dummy::default()).unwrap();

            let mut group = NodeGroup::new("inner group");
            let node_b = group.add_node(Dummy::default()).unwrap();
            let node_c = group.add_node(Dummy::default()).unwrap();
            group
                .connect_nodes(node_b, "output_1", node_c, "input_1", meter!(0.1))
                .unwrap();
            group.map_input_port(node_b, "input_1", "g_in").unwrap();
            group.map_output_port(node_c, "output_1", "g_out").unwrap();
            let group_id = scenery.add_node(group).unwrap();

            scenery
                .connect_nodes(node_a, "output_1", group_id, "g_in", meter!(0.1))
                .unwrap();
            scenery
                .connect_nodes(group_id, "g_out", node_d, "input_1", meter!(0.1))
                .unwrap();

            // A reference at the root pointing at the nested B - it must survive the uuid-preserving cut.
            let node_b_ref = scenery.node_recursive(node_b).unwrap().0;
            let node_reference = NodeReference::from_node(&node_b_ref).unwrap();
            let ref_id = scenery.add_node(node_reference).unwrap();

            (root_id, group_id, node_a, node_b, node_c, ref_id)
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
        nodes_to_copy.insert(node_b);
        let req = test::TestRequest::post()
            .uri("/copy_nodes")
            .set_json(&nodes_to_copy)
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );

        // Cut B out of the group into the root. Before the fix this 400'd; it must now succeed.
        let req = test::TestRequest::post()
            .uri("/cut_nodes")
            .set_json(&(root_id, (500.0, 500.0)))
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "cutting a connected node out of a nested group must not error"
        );

        let parent_of = |id: Uuid| {
            app_state
                .document
                .lock()
                .scenery()
                .node_recursive(id)
                .unwrap()
                .1
        };
        let g_in_maps_b = |app_state: &Data<AppState>| {
            app_state
                .document
                .lock()
                .scenery()
                .with_group_node(group_id, |g| {
                    !g.graph()
                        .port_map(&PortType::Input)
                        .assigned_ports_for_node(node_b)
                        .is_empty()
                })
                .unwrap()
        };
        let a_to_g_edge = |app_state: &Data<AppState>| {
            app_state
                .document
                .lock()
                .scenery()
                .graph()
                .connections()
                .iter()
                .any(|c| c.src_id == node_a && c.target_id == group_id)
        };
        let b_to_c_edge = |app_state: &Data<AppState>| {
            app_state
                .document
                .lock()
                .scenery()
                .with_group_node(group_id, NodeGroup::connections)
                .unwrap()
                .iter()
                .any(|c| c.src_id == node_b && c.target_id == node_c)
        };

        // After the cut: B is bare at the root, and G's exposing mapping + the A -> G edge + the B -> C
        // edge are all cascade-deleted (the graph is consistent - the "can't delete the mapping" symptom
        // is gone).
        assert_eq!(parent_of(node_b), root_id, "B must have moved to the root");
        assert!(
            !g_in_maps_b(&app_state),
            "G's g_in mapping for B must be cascade-deleted"
        );
        assert!(
            !a_to_g_edge(&app_state),
            "the A -> G edge that consumed g_in must be cascade-deleted"
        );
        assert!(!b_to_c_edge(&app_state), "the B -> C edge must be dropped");
        let ref_target = app_state
            .document
            .lock()
            .scenery()
            .with_node_attr(ref_id, |attr| {
                attr.properties().get("reference id").cloned()
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            ref_target,
            Proptype::Uuid(node_b),
            "the reference must still resolve to B's unchanged uuid"
        );

        // Undo restores everything: B back in G, with B -> C, the g_in mapping, and the A -> G edge.
        assert_eq!(
            app.call(test::TestRequest::post().uri("/undo").to_request())
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(parent_of(node_b), group_id, "undo must put B back in G");
        assert!(b_to_c_edge(&app_state), "undo must restore B -> C");
        assert!(
            g_in_maps_b(&app_state),
            "undo must restore G's g_in mapping"
        );
        assert!(a_to_g_edge(&app_state), "undo must restore the A -> G edge");

        // Redo cascade-deletes the links again and moves B back out.
        assert_eq!(
            app.call(test::TestRequest::post().uri("/redo").to_request())
                .await
                .unwrap()
                .status(),
            StatusCode::OK,
            "redo of the cascade-deleting cut must succeed"
        );
        assert_eq!(
            parent_of(node_b),
            root_id,
            "redo must move B back to the root"
        );
        assert!(
            !g_in_maps_b(&app_state),
            "redo must remove G's g_in mapping again"
        );
        assert!(
            !a_to_g_edge(&app_state),
            "redo must remove the A -> G edge again"
        );
        assert!(!b_to_c_edge(&app_state), "redo must drop B -> C again");
    }

    /// Regression test for the same-group half of the cut/paste link-handling bug: cutting a node that
    /// has a connection to a sibling *not* included in the cut must sever that link, even though the
    /// node is pasted straight back into the same group it came from. Before the fix, a same-group cut
    /// ran no connection-severing logic at all (the node was filtered out of the relocation map purely
    /// because its parent already equals the paste target) and kept every link regardless of whether the
    /// other endpoint was cut too.
    #[actix_web::test]
    async fn test_cut_node_severs_link_to_uncut_sibling_same_group() {
        use opossum_core::nodes::Dummy;

        let app_state = Data::new(AppState::default());
        let (root_id, node_a, node_b) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();
            let node_a = scenery.add_node(Dummy::default()).unwrap();
            let node_b = scenery.add_node(Dummy::default()).unwrap();
            scenery
                .connect_nodes(node_a, "output_1", node_b, "input_1", meter!(0.1))
                .unwrap();
            (root_id, node_a, node_b)
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

        let a_to_b_edge = |app_state: &Data<AppState>| {
            app_state
                .document
                .lock()
                .scenery()
                .graph()
                .connections()
                .iter()
                .any(|c| c.src_id == node_a && c.target_id == node_b)
        };
        assert!(
            a_to_b_edge(&app_state),
            "sanity: A -> B must exist before the cut"
        );

        // Cut only A, paste it back into the root at a new position - B stays behind, uncut.
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

        assert!(
            !a_to_b_edge(&app_state),
            "A -> B must be severed - B was not part of the cut, even though A lands back in the same group"
        );

        // Undo restores the severed link.
        assert_eq!(
            app.call(test::TestRequest::post().uri("/undo").to_request())
                .await
                .unwrap()
                .status(),
            StatusCode::OK,
            "undo of the same-group cut must succeed"
        );
        assert!(a_to_b_edge(&app_state), "undo must restore A -> B");

        // Redo severs it again.
        assert_eq!(
            app.call(test::TestRequest::post().uri("/redo").to_request())
                .await
                .unwrap()
                .status(),
            StatusCode::OK,
            "redo of the same-group cut must succeed"
        );
        assert!(!a_to_b_edge(&app_state), "redo must sever A -> B again");
    }

    /// Regression test for the cross-group half of the cut/paste link-handling bug: cutting *two*
    /// connected nodes together out of a group must keep the connection between them, even though both
    /// leave their group. Before the fix, a cross-group cut cascade-deleted every connection touching
    /// either moved node via a blanket "any edge with this endpoint" capture, with no concept of "the
    /// other endpoint was cut too" - so the link between two co-cut nodes was dropped exactly like a
    /// link to an uncut sibling.
    #[actix_web::test]
    async fn test_cut_two_connected_nodes_preserves_link_cross_group() {
        use opossum_core::nodes::{Dummy, NodeGroup};

        let app_state = Data::new(AppState::default());
        let (root_id, group_id, node_b, node_c) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();

            let mut group = NodeGroup::new("inner group");
            let node_b = group.add_node(Dummy::default()).unwrap();
            let node_c = group.add_node(Dummy::default()).unwrap();
            group
                .connect_nodes(node_b, "output_1", node_c, "input_1", meter!(0.1))
                .unwrap();
            let group_id = scenery.add_node(group).unwrap();

            (root_id, group_id, node_b, node_c)
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

        // Cut both B and C together, out of the group into the root.
        let mut nodes_to_copy = HashSet::new();
        nodes_to_copy.insert(node_b);
        nodes_to_copy.insert(node_c);
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

        let b_to_c_edge_at_root = |app_state: &Data<AppState>| {
            app_state
                .document
                .lock()
                .scenery()
                .graph()
                .connections()
                .iter()
                .any(|c| c.src_id == node_b && c.target_id == node_c)
        };
        assert!(
            b_to_c_edge_at_root(&app_state),
            "B -> C must survive the move - both nodes were cut together"
        );

        // Undo moves B and C back into the group, still connected.
        assert_eq!(
            app.call(test::TestRequest::post().uri("/undo").to_request())
                .await
                .unwrap()
                .status(),
            StatusCode::OK,
            "undo of the cross-group cut must succeed"
        );
        let b_to_c_edge_in_group = |app_state: &Data<AppState>| {
            app_state
                .document
                .lock()
                .scenery()
                .with_group_node(group_id, NodeGroup::connections)
                .unwrap()
                .iter()
                .any(|c| c.src_id == node_b && c.target_id == node_c)
        };
        assert!(
            b_to_c_edge_in_group(&app_state),
            "undo must restore B -> C inside the group"
        );

        // Redo moves them back out, still connected.
        assert_eq!(
            app.call(test::TestRequest::post().uri("/redo").to_request())
                .await
                .unwrap()
                .status(),
            StatusCode::OK,
            "redo of the cross-group cut must succeed"
        );
        assert!(
            b_to_c_edge_at_root(&app_state),
            "redo must restore B -> C at the root"
        );
    }

    /// Regression test for the same-group cut missing its own port-map cascade: a node whose port is
    /// exposed via its own group's external port map (consumed by a connection one level up) must have
    /// that mapping - and the connection consuming it - severed by a cut, even when the node is pasted
    /// straight back into the same group. Before the fix, the same-group branch never called any
    /// cascade-teardown logic at all.
    #[actix_web::test]
    async fn test_cut_node_severs_own_exposed_port_mapping_same_group() {
        use opossum_core::{
            nodes::{Dummy, NodeGroup},
            prelude::PortType,
        };

        let app_state = Data::new(AppState::default());
        let (group_id, node_a, node_b) = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();

            let node_a = scenery.add_node(Dummy::default()).unwrap();

            let mut group = NodeGroup::new("inner group");
            let node_b = group.add_node(Dummy::default()).unwrap();
            group.map_input_port(node_b, "input_1", "g_in").unwrap();
            let group_id = scenery.add_node(group).unwrap();

            scenery
                .connect_nodes(node_a, "output_1", group_id, "g_in", meter!(0.1))
                .unwrap();

            (group_id, node_a, node_b)
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

        let g_in_maps_b = |app_state: &Data<AppState>| {
            app_state
                .document
                .lock()
                .scenery()
                .with_group_node(group_id, |g| {
                    !g.graph()
                        .port_map(&PortType::Input)
                        .assigned_ports_for_node(node_b)
                        .is_empty()
                })
                .unwrap()
        };
        let a_to_g_edge = |app_state: &Data<AppState>| {
            app_state
                .document
                .lock()
                .scenery()
                .graph()
                .connections()
                .iter()
                .any(|c| c.src_id == node_a && c.target_id == group_id)
        };
        assert!(
            g_in_maps_b(&app_state),
            "sanity: g_in must map B before the cut"
        );
        assert!(
            a_to_g_edge(&app_state),
            "sanity: A -> G must exist before the cut"
        );

        // Cut B and paste it back into the same group G.
        let mut nodes_to_copy = HashSet::new();
        nodes_to_copy.insert(node_b);
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
            .set_json(&(group_id, (50.0, 50.0)))
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "cutting B back into its own group must not error"
        );

        assert!(
            !g_in_maps_b(&app_state),
            "G's g_in mapping for B must be cascade-deleted by the same-group cut"
        );
        assert!(
            !a_to_g_edge(&app_state),
            "the A -> G edge that consumed g_in must be cascade-deleted too"
        );

        // Undo restores the mapping and the edge it fed.
        assert_eq!(
            app.call(test::TestRequest::post().uri("/undo").to_request())
                .await
                .unwrap()
                .status(),
            StatusCode::OK,
            "undo of the same-group cascade cut must succeed"
        );
        assert!(
            g_in_maps_b(&app_state),
            "undo must restore G's g_in mapping"
        );
        assert!(a_to_g_edge(&app_state), "undo must restore the A -> G edge");
    }

    /// Regression test for the bug where nothing stopped a reference to a group from being cut into that
    /// very group - which deadlocks the analyzer (analyzing a group holds its `Mutex` for the duration of
    /// its own recursive descent, so a reference resolving back to an already-locked ancestor
    /// self-deadlocks). Builds `G` and a sibling `R = ref(G)` at the root, copies `R`, then asserts cutting
    /// it into `G` is rejected and `R` stays at the root.
    #[actix_web::test]
    async fn test_cut_reference_into_own_target_is_rejected() {
        use opossum_core::nodes::NodeGroup;

        let app_state = Data::new(AppState::default());
        let (root_id, g_id, ref_id) = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();
            let root_id = scenery.node_attr().uuid();
            let g_id = scenery.add_node(NodeGroup::new("G")).unwrap();
            let g_ref = scenery.node(g_id).unwrap();
            let node_reference = NodeReference::from_node(&g_ref).unwrap();
            let ref_id = scenery.add_node(node_reference).unwrap();
            (root_id, g_id, ref_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_copy_nodes)
                .service(post_cut_nodes),
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
            .uri("/cut_nodes")
            .set_json(&(g_id, (50.0, 50.0)))
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::BAD_REQUEST,
            "cutting a reference into its own target group must be rejected"
        );
        assert!(
            app_state
                .document
                .lock()
                .scenery()
                .node_recursive(ref_id)
                .is_ok_and(|(_, parent)| parent == root_id),
            "the rejected reference must remain at its original location"
        );
    }

    /// Same hazard, one level deeper: `G1` contains `G2`; a reference to `G1` sitting at the root must
    /// also be rejected when cut into `G2`, since `G2` lives inside `G1`'s own subtree too.
    #[actix_web::test]
    async fn test_cut_reference_into_nested_descendant_of_target_is_rejected() {
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
                .service(post_cut_nodes),
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
            .uri("/cut_nodes")
            .set_json(&(g2_id, (50.0, 50.0)))
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::BAD_REQUEST,
            "cutting a reference into a nested descendant of its target must be rejected"
        );
    }

    /// A reference and its own target cut together, as siblings, into an unrelated destination group must
    /// still succeed - they keep the same (valid) relative structure either way, unlike the two rejected
    /// cases above.
    #[actix_web::test]
    async fn test_cut_reference_and_target_together_as_siblings_is_allowed() {
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
                .service(post_cut_nodes),
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
            .uri("/cut_nodes")
            .set_json(&(dest_id, (50.0, 50.0)))
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "cutting a reference together with its own target as siblings must still be allowed"
        );
    }
}
