use actix_web::{
    post,
    web::{self, Json},
};
use opossum_core::types::api_types::{ErrorResponse, MoveNodesRequest, MoveNodesResponse};

use crate::{
    app_state::AppState,
    error::BackEndErrorResponse,
    helper_functions::{lowest_common_ancestor_group, relocate_nodes_in_document},
    undo::{Command, MoveNodes},
};

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
pub(crate) async fn post_move_nodes(
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

#[cfg(test)]
mod test {
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use opossum_core::{core_optics::node_attr::HasNodeAttr, meter};

    use super::*;
    use crate::{
        app_state::AppState,
        document::{redo_document, undo_document},
    };

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
}
