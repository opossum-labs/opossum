use actix_web::{
    HttpResponse, delete, get, post,
    web::{self},
};
use opossum_core::{
    error::OpossumError,
    prelude::{OpticNode, PortType},
    types::api_types::{
        AddPortMappingRequest, ErrorResponse, PortMappingsResponse, PortNamesResponse,
        RemovePortMapQuery, RemovePortMapResponse,
    },
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    error::BackEndErrorResponse,
    helper_functions::{map_port, remove_port_map_cascade},
    undo::{Command, RemovePortMap},
};

/// Get the port mappings of a group node
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose portmaps should be sent"),
    ),
    responses(
        (status = OK, description = "Node portmaps successfully sent!", body = PortMappingsResponse, content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/port_mappings")]
pub async fn get_port_mappings(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let group_id = path.into_inner();
    let (inputs, outputs) =
        data.document
            .lock()
            .scenery_mut()
            .with_group_node_mut(group_id, |g| {
                (
                    g.graph().port_map(&PortType::Input).clone(),
                    g.graph().port_map(&PortType::Output).clone(),
                )
            })?;

    let response = PortMappingsResponse { inputs, outputs };
    Ok(HttpResponse::Ok().json(response))
}

/// Map a port of an internal node to a port of the group node.
///
/// This will create a new port on the group node and connect it to the internal node's port. The new port will be named as specified in the request.
/// If a port with the same name already exists on the group node, an error will be returned. This function will also return the updated lists of mapped
/// input and output ports of the group node, which can be used to update the UI accordingly.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose port should be mapped"),
    ),
    request_body = AddPortMappingRequest,
    responses(
        (status = CREATED, description = "Node port successfully mapped", body = PortNamesResponse, content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/{uuid}/port_mappings")]
pub async fn post_port_mapping(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    port_mapping_request: web::Json<AddPortMappingRequest>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let group_id = path.into_inner();
    let pmap_inf = port_mapping_request.into_inner();

    let (inputs, outputs) =
        data.document
            .lock()
            .scenery_mut()
            .with_group_node_mut(group_id, |g| {
                map_port(
                    g,
                    pmap_inf.port_type,
                    pmap_inf.internal_node_id,
                    &pmap_inf.internal_port_name,
                    &pmap_inf.external_port_name,
                )?;

                let ports = g.ports();
                let inputs: Vec<String> = ports.ports(&PortType::Input).keys().cloned().collect();
                let outputs: Vec<String> = ports.ports(&PortType::Output).keys().cloned().collect();

                Ok::<(Vec<String>, Vec<String>), OpossumError>((inputs, outputs)) // <-- OpossumError statt BackEndErrorResponse!
            })??;

    let (_, parent_group_id) = data.document.lock().scenery().node_recursive(group_id)?;
    data.push_undo(Command::RemovePortMap(RemovePortMap {
        group_id,
        parent_group_id,
        query: RemovePortMapQuery {
            external_port_name: pmap_inf.external_port_name.clone(),
            port_type: pmap_inf.port_type,
        },
    }));

    let response = PortNamesResponse { inputs, outputs };
    Ok(HttpResponse::Created().json(response)) // 201 Created
}

/// Remove a port mapping from a group
#[utoipa::path(
    tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose port-map should be removed"),
        RemovePortMapQuery
    ),
    responses(
        (status = OK, description = "Node port successfully removed", body = RemovePortMapResponse, content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[allow(clippy::significant_drop_tightening)]
#[delete("/{uuid}/port_mappings")]
pub async fn remove_port_map(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    query: web::Query<RemovePortMapQuery>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let group_id = path.into_inner();
    let RemovePortMapQuery {
        external_port_name,
        port_type,
    } = query.into_inner();

    let mut document = data.document.lock();
    let scenery = document.scenery_mut();

    let Some(cascade) = remove_port_map_cascade(scenery, group_id, &external_port_name, port_type)?
    else {
        let response = RemovePortMapResponse {
            port_removed: false,
            removed_port_mappings: Vec::new(),
            disconnected_connections: Vec::new(),
        };
        return Ok(HttpResponse::Ok().json(response));
    };

    // One removal = one undo step - see `From<&PortMapCascadeRemoval> for Command` for how the
    // restore batch is ordered and why.
    data.push_undo(Command::from(&cascade));

    let removed_port_mappings = cascade
        .levels
        .into_iter()
        .map(|level| {
            (
                level.group_id,
                level.internal_node_id,
                level.external_port_name,
                level.port_type,
            )
        })
        .collect();

    let response = RemovePortMapResponse {
        port_removed: true,
        removed_port_mappings,
        disconnected_connections: cascade.disconnected_connections,
    };

    Ok(HttpResponse::Ok().json(response)) // 200 OK (Daten werden zurückgegeben)
}

#[cfg(test)]
mod test {
    use super::*;
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};

    fn create_test_state() -> Data<AppState> {
        Data::new(AppState::default())
    }

    #[actix_web::test]
    async fn test_get_port_mappings_invalid_uuid() {
        let app_state = create_test_state();
        let app =
            test::init_service(App::new().app_data(app_state).service(get_port_mappings)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/{}/port_mappings", Uuid::new_v4()))
            .to_request();

        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_remove_port_map_invalid_uuid() {
        let app_state = create_test_state();
        let app = test::init_service(App::new().app_data(app_state).service(remove_port_map)).await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/{}/port_mappings?external_port_name=out&port_type=Output",
                Uuid::new_v4()
            ))
            .to_request();

        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    /// Removing one port mapping must only disconnect the external connection that used
    /// that specific port, not every external connection to the group.
    #[actix_web::test]
    async fn test_remove_port_map_only_removes_matching_connection() {
        use opossum_core::{
            core_optics::node_attr::HasNodeAttr,
            meter,
            nodes::{Dummy, NodeGroup},
        };

        let app_state = create_test_state();
        let (root_id, group_id, node_a, ext_node_a, ext_node_b) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();

            let mut group = NodeGroup::new("inner group");
            let n1 = group.add_node(Dummy::default()).unwrap();
            let n2 = group.add_node(Dummy::default()).unwrap();
            group.map_input_port(n1, "input_1", "ext_in_1").unwrap();
            group.map_input_port(n2, "input_1", "ext_in_2").unwrap();

            let group_id = scenery.add_node(group).unwrap();
            let ext_node_a = scenery.add_node(Dummy::default()).unwrap();
            let ext_node_b = scenery.add_node(Dummy::default()).unwrap();

            scenery
                .connect_nodes(ext_node_a, "output_1", group_id, "ext_in_1", meter!(0.1))
                .unwrap();
            scenery
                .connect_nodes(ext_node_b, "output_1", group_id, "ext_in_2", meter!(0.1))
                .unwrap();

            (root_id, group_id, n1, ext_node_a, ext_node_b)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(remove_port_map),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/{group_id}/port_mappings?external_port_name=ext_in_1&port_type=Input"
            ))
            .to_request();

        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body: RemovePortMapResponse = test::read_body_json(resp).await;
        assert!(body.port_removed);
        assert_eq!(
            body.removed_port_mappings,
            vec![(group_id, node_a, "ext_in_1".to_string(), PortType::Input)],
            "exactly the requested single-level mapping must be reported removed - the group has \
             no further chain to walk, so the cascade stops after 1 level"
        );
        assert_eq!(body.disconnected_connections.len(), 1);
        assert_eq!(body.disconnected_connections[0].0, root_id);
        assert_eq!(body.disconnected_connections[0].1.src_uuid(), ext_node_a);
        assert_eq!(body.disconnected_connections[0].1.target_uuid(), group_id);
        assert_eq!(body.disconnected_connections[0].1.target_port(), "ext_in_1");

        // the connection to the *other* mapped port must still be intact
        let document = app_state.document.lock();
        let remaining = document
            .scenery()
            .graph()
            .get_connection_info_of_node(group_id);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].src_id, ext_node_b);
        assert_eq!(remaining[0].target_id, group_id);
        assert_eq!(remaining[0].target_port, "ext_in_2");
    }

    /// Regression test for the bug where removing a port mapping chained through nested groups
    /// only removed the innermost entry, leaving the rest of the chain (and the live connection
    /// it ultimately depended on) dangling. Builds `root { G2 { G1 { L } } }`: `L`'s output is
    /// mapped to `G1`'s own external port `g1_ext_out`; `G1` itself is a member of `G2`, which
    /// maps `g1_ext_out` to `G2`'s own external port `g2_ext_out`; `G2`'s `g2_ext_out` is
    /// connected to sibling `N` at the root. Removes the innermost mapping (on `G1`, exposing
    /// `L`) and asserts the response reports both mapping levels removed (innermost first) and
    /// the root-level `G2 -> N` connection disconnected, all three actually gone from the
    /// document, and a single undo restores the entire chain.
    #[actix_web::test]
    async fn test_remove_port_map_cascades_through_nested_groups() {
        use crate::document::undo_document;
        use opossum_core::{
            core_optics::node_attr::HasNodeAttr,
            meter,
            nodes::{Dummy, NodeGroup},
        };

        let app_state = create_test_state();
        let (root_id, g1_id, g2_id, lens, n) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();

            let mut g1 = NodeGroup::new("G1");
            let lens = g1.add_node(Dummy::default()).unwrap();
            g1.map_output_port(lens, "output_1", "g1_ext_out").unwrap();

            let mut g2 = NodeGroup::new("G2");
            let g1_id = g2.add_node(g1).unwrap();
            g2.map_output_port(g1_id, "g1_ext_out", "g2_ext_out")
                .unwrap();

            let g2_id = scenery.add_node(g2).unwrap();
            let n = scenery.add_node(Dummy::default()).unwrap();
            scenery
                .connect_nodes(g2_id, "g2_ext_out", n, "input_1", meter!(0.1))
                .unwrap();

            (root_id, g1_id, g2_id, lens, n)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(remove_port_map)
                .service(undo_document),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/{g1_id}/port_mappings?external_port_name=g1_ext_out&port_type=Output"
            ))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body: RemovePortMapResponse = test::read_body_json(resp).await;
        assert!(body.port_removed);
        assert_eq!(
            body.removed_port_mappings,
            vec![
                (g1_id, lens, "g1_ext_out".to_string(), PortType::Output),
                (g2_id, g1_id, "g2_ext_out".to_string(), PortType::Output),
            ],
            "both chained levels must be reported removed, innermost (G1) first"
        );
        assert_eq!(body.disconnected_connections.len(), 1);
        assert_eq!(body.disconnected_connections[0].0, root_id);
        assert_eq!(body.disconnected_connections[0].1.src_uuid(), g2_id);
        assert_eq!(body.disconnected_connections[0].1.target_uuid(), n);

        {
            let document = app_state.document.lock();
            let g1_mapping = document
                .scenery()
                .with_group_node(g1_id, |g| {
                    g.graph()
                        .port_map(&PortType::Output)
                        .get("g1_ext_out")
                        .cloned()
                })
                .unwrap();
            assert_eq!(g1_mapping, None, "G1's own mapping must be gone");
            let g2_mapping = document
                .scenery()
                .with_group_node(g2_id, |g| {
                    g.graph()
                        .port_map(&PortType::Output)
                        .get("g2_ext_out")
                        .cloned()
                })
                .unwrap();
            assert_eq!(g2_mapping, None, "G2's chained mapping must be gone too");
            let root_connections = document.scenery().graph().connections();
            assert!(
                !root_connections
                    .iter()
                    .any(|c| c.src_id == g2_id && c.target_id == n),
                "the root-level G2 -> N connection must be gone"
            );
        }

        let req = test::TestRequest::post().uri("/undo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "a single undo must restore the entire chain"
        );

        let document = app_state.document.lock();
        let g1_mapping = document
            .scenery()
            .with_group_node(g1_id, |g| {
                g.graph()
                    .port_map(&PortType::Output)
                    .get("g1_ext_out")
                    .cloned()
            })
            .unwrap();
        assert_eq!(g1_mapping, Some((lens, "output_1".to_string())));
        let g2_mapping = document
            .scenery()
            .with_group_node(g2_id, |g| {
                g.graph()
                    .port_map(&PortType::Output)
                    .get("g2_ext_out")
                    .cloned()
            })
            .unwrap();
        assert_eq!(g2_mapping, Some((g1_id, "g1_ext_out".to_string())));
        let root_connections = document.scenery().graph().connections();
        assert!(
            root_connections
                .iter()
                .any(|c| c.src_id == g2_id && c.target_id == n),
            "the root-level G2 -> N connection must be restored"
        );
    }

    /// Regression test for the "orphaned top of chain" case: if the outermost group in a mapping
    /// chain has no live connection consuming it (nothing wired to it yet), the cascade must still
    /// remove every chained mapping level - there's just nothing to disconnect at the end. Same
    /// `G2 { G1 { L } }` setup as the connected case, but `G2`'s own `g2_ext_out` is never
    /// connected to anything.
    #[actix_web::test]
    async fn test_remove_port_map_cascade_with_no_live_connection_at_top() {
        use opossum_core::nodes::{Dummy, NodeGroup};

        let app_state = create_test_state();
        let (g1_id, g2_id, lens) = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();

            let mut g1 = NodeGroup::new("G1");
            let lens = g1.add_node(Dummy::default()).unwrap();
            g1.map_output_port(lens, "output_1", "g1_ext_out").unwrap();

            let mut g2 = NodeGroup::new("G2");
            let g1_id = g2.add_node(g1).unwrap();
            g2.map_output_port(g1_id, "g1_ext_out", "g2_ext_out")
                .unwrap();

            let g2_id = scenery.add_node(g2).unwrap();

            (g1_id, g2_id, lens)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(remove_port_map),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/{g1_id}/port_mappings?external_port_name=g1_ext_out&port_type=Output"
            ))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body: RemovePortMapResponse = test::read_body_json(resp).await;
        assert!(body.port_removed);
        assert_eq!(
            body.removed_port_mappings,
            vec![
                (g1_id, lens, "g1_ext_out".to_string(), PortType::Output),
                (g2_id, g1_id, "g2_ext_out".to_string(), PortType::Output),
            ],
            "both chained levels must still be removed even with nothing to disconnect at the top"
        );
        assert!(
            body.disconnected_connections.is_empty(),
            "nothing was ever connected to G2's own export, so nothing should be disconnected"
        );
    }

    /// Regression test for the bug where undo/redo of a multi-level port-map cascade could jump to a
    /// *different* tab each direction, because `Command::Batch::apply` reverses its sub-commands' order
    /// and the client used list order to pick the focus tab. Now the backend names the focus target
    /// authoritatively via [`crate::undo::Command::jump_target`], whose batch handling picks by a stable,
    /// order-invariant key. Same `root { G2 { G1 { L } } }` fixture as
    /// [`test_remove_port_map_cascades_through_nested_groups`]; asserts undo and redo resolve to the *same*
    /// group (one of the cascade's own).
    #[actix_web::test]
    async fn test_remove_port_map_cascade_undo_redo_report_same_origin() {
        use crate::document::{redo_document, undo_document};
        use opossum_core::{
            nodes::{Dummy, NodeGroup},
            types::api_types::UndoRedoResponse,
        };

        let app_state = create_test_state();
        let (g1_id, g2_id) = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();

            let mut g1 = NodeGroup::new("G1");
            let lens = g1.add_node(Dummy::default()).unwrap();
            g1.map_output_port(lens, "output_1", "g1_ext_out").unwrap();

            let mut g2 = NodeGroup::new("G2");
            let g1_id = g2.add_node(g1).unwrap();
            g2.map_output_port(g1_id, "g1_ext_out", "g2_ext_out")
                .unwrap();

            let g2_id = scenery.add_node(g2).unwrap();

            (g1_id, g2_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(remove_port_map)
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/{g1_id}/port_mappings?external_port_name=g1_ext_out&port_type=Output"
            ))
            .to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);

        let undo_req = test::TestRequest::post().uri("/undo").to_request();
        let undo_resp = app.call(undo_req).await.unwrap();
        assert_eq!(undo_resp.status(), StatusCode::OK);
        let undo_body: UndoRedoResponse = test::read_body_json(undo_resp).await;

        let redo_req = test::TestRequest::post().uri("/redo").to_request();
        let redo_resp = app.call(redo_req).await.unwrap();
        assert_eq!(redo_resp.status(), StatusCode::OK);
        let redo_body: UndoRedoResponse = test::read_body_json(redo_resp).await;

        // `Command::jump_target` focuses the cascade's origin - the group whose own mapping was directly
        // removed (G1) - not the re-exposing ancestor (G2). And it must be the *same* group on undo and
        // redo, despite `Command::Batch` reversing its sub-commands' order between the two directions (the
        // original bug: list order was the only signal, so the focus flipped between G1 and G2).
        let undo_jump = undo_body
            .jump
            .expect("undo of a port-map cascade must carry a jump target");
        let redo_jump = redo_body
            .jump
            .expect("redo of a port-map cascade must carry a jump target");
        assert_eq!(
            undo_jump.graph_id, g1_id,
            "undo must focus G1, the group whose own mapping was directly removed, not the ancestor G2"
        );
        assert_eq!(
            redo_jump.graph_id, g1_id,
            "redo must focus the same G1 despite Command::Batch reversing its sub-commands' order"
        );
    }

    /// Regression test for the jump drifting one level up on the *second* undo of a nested port-map
    /// cascade with a live connection. `apply_remove_port_map` always returns a `Batch` (even for its one
    /// level), so after a redo the next undo command nests its `AddPortMap`s inside per-command `Batch`es;
    /// `port_map_cascade_origin` only scanned top-level commands, found none, and `batch_jump_target` fell
    /// back to an arbitrary tab. It now recurses into nested `Batch`es. Builds `root { G2 { G1 { lens } },
    /// detector }` with a live connection `G2.g2_ext_out -> detector`, deletes G1's own mapping, and asserts
    /// undo, redo, AND a second undo all focus G1 (the group whose own mapping was removed).
    #[actix_web::test]
    async fn test_remove_port_map_cascade_second_undo_stays_on_origin() {
        use crate::document::{redo_document, undo_document};
        use opossum_core::{
            meter,
            nodes::{Dummy, NodeGroup},
            types::api_types::UndoRedoResponse,
        };

        let app_state = create_test_state();
        let g1_id = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();

            let mut g1 = NodeGroup::new("G1");
            let lens = g1.add_node(Dummy::default()).unwrap();
            g1.map_output_port(lens, "output_1", "g1_ext_out").unwrap();

            let mut g2 = NodeGroup::new("G2");
            let g1_id = g2.add_node(g1).unwrap();
            g2.map_output_port(g1_id, "g1_ext_out", "g2_ext_out")
                .unwrap();

            let g2_id = scenery.add_node(g2).unwrap();

            // A live connection at root through the whole chain (the user's connected-nodes case), so the
            // cascade also disconnects an edge - the shape that produced the nested batches on undo#2.
            let detector = scenery.add_node(Dummy::default()).unwrap();
            scenery
                .connect_nodes(g2_id, "g2_ext_out", detector, "input_1", meter!(0.1))
                .unwrap();

            g1_id
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(remove_port_map)
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        let del_req = test::TestRequest::delete()
            .uri(&format!(
                "/{g1_id}/port_mappings?external_port_name=g1_ext_out&port_type=Output"
            ))
            .to_request();
        assert_eq!(app.call(del_req).await.unwrap().status(), StatusCode::OK);

        // undo, redo, undo again - each must focus the innermost origin G1, never drifting.
        let jump_graph_id = |body: UndoRedoResponse| {
            body.jump
                .expect("an undo/redo of a port-map cascade must carry a jump target")
                .graph_id
        };
        for (uri, label) in [
            ("/undo", "undo"),
            ("/redo", "redo"),
            ("/undo", "second undo"),
        ] {
            let resp = app
                .call(test::TestRequest::post().uri(uri).to_request())
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK, "{label} must succeed");
            let body: UndoRedoResponse = test::read_body_json(resp).await;
            assert_eq!(
                jump_graph_id(body),
                g1_id,
                "{label} must focus the innermost origin G1, not drift a level up"
            );
        }
    }
}
