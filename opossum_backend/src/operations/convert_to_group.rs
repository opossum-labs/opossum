use actix_web::{
    post,
    web::{self, Json},
};
use opossum_core::{
    error::{OpmResult, OpossumError},
    nodes::NodeGroup,
    prelude::PortType,
    types::api_types::{ConnectInfo, ConvertToGroupRequest, ConvertToGroupResponse, ErrorResponse},
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    error::BackEndErrorResponse,
    helper_functions::{
        collect_group_connections, collect_node_refs_and_pos, create_new_group_node_info,
        relocate_nodes_in_document, split_sort_connections,
    },
    undo::{Command, GroupConversion, ReroutedMapping},
};

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

#[cfg(test)]
mod test {
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use nalgebra::Point2;
    use opossum_core::{core_optics::node_attr::HasNodeAttr, meter, utils::LockExt};

    use super::*;
    use crate::{
        app_state::AppState,
        document::{redo_document, undo_document},
    };

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
        use opossum_core::nodes::{Dummy, NodeGroup};

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
        use opossum_core::nodes::{Dummy, NodeGroup};

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
        use opossum_core::nodes::{Dummy, NodeGroup};

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
