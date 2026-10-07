use std::collections::HashMap;

use actix_web::{
    post,
    web::{self, Json},
};
use nalgebra::Point2;
use opossum_core::{
    core_optics::{NodeAttrExt, OpticRef, node_attr::HasNodeAttr},
    error::OpossumError,
    meter,
    nodes::{ConnectionInfo, GraphDelta, NodeGroup, NodeReference, create_node_ref},
    opm_document::{AnalyzerInfo, OpmDocument},
    prelude::{OpticNode, PortMap, PortType, Proptype},
    types::api_types::{AnalyzerItemDto, ConnectInfo, ErrorResponse, NodeInfo, PasteNodesResponse},
};
use uuid::Uuid;

use super::upper_left_corner_of_nodes;
use crate::{
    app_state::{AppState, NodeCacheItem},
    error::BackEndErrorResponse,
    helper_functions::{
        build_connect_info, map_port, parent_group_id_or_self, validate_relocated_references,
    },
    undo::{Command, PatchAmplifierNodes, PatchPumpScenario},
};

/// The pasted-in node/connection info [`insert_copied_nodes`] hands back to [`post_paste_nodes`].
struct PastedNodes {
    grouped_node_infos: HashMap<Uuid, Vec<NodeInfo>>,
    grouped_connect_info: HashMap<Uuid, Vec<ConnectInfo>>,
    /// Maps each copied node's original uuid to the fresh one its paste got.
    node_id_link: HashMap<Uuid, Uuid>,
}

/// Copies `copied_optical_nodes` into `paste_group_id` (recursively, preserving group structure),
/// replays their port maps and reference targets, and reconnects their captured internal connections
/// through the fresh uuids.
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
                node_info.push(NodeInfo::from_analyzable(&**node_ref, None));
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
        node_id_link,
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
            NodeCacheItem::Analyzer(a) => analyzer.push(*a),
        }
    }
    (optical, analyzer)
}

/// Carries a pasted node's amplifier candidacy and its gain model in every pump scenario over to its
/// fresh uuid.
fn propagate_amplifier_state(
    document: &mut OpmDocument,
    node_id_link: &HashMap<Uuid, Uuid>,
) -> Vec<Command> {
    let mut inverses = Vec::new();

    let candidates_before = document.amplifier_nodes().clone();
    for (old_id, new_id) in node_id_link {
        if document.is_amplifier_node(*old_id) {
            document.set_is_amplifier_node(*new_id, true);
        }
    }
    if *document.amplifier_nodes() != candidates_before {
        inverses.push(Command::PatchAmplifierNodes(PatchAmplifierNodes {
            old: document.amplifier_nodes().clone(),
            new: candidates_before,
        }));
    }

    let scenario_ids: Vec<Uuid> = document.pump_scenarios().keys().copied().collect();
    for scenario_id in scenario_ids {
        let Some(before) = document.pump_scenario(scenario_id).cloned() else {
            continue;
        };
        let Some(scenario) = document.pump_scenario_mut(scenario_id) else {
            continue;
        };
        for (old_id, new_id) in node_id_link {
            scenario.set_config(*new_id, before.config(*old_id));
        }
        if *scenario != before {
            inverses.push(Command::PatchPumpScenario(PatchPumpScenario {
                id: scenario_id,
                old: scenario.clone(),
                new: before,
            }));
        }
    }

    inverses
}

/// Paste copied nodes
///
/// This function duplicates the nodes/analyzers currently in the copy cache into the target group,
/// minting a fresh uuid for each copy.
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
    let min_pos = upper_left_corner_of_nodes(&copied_nodes);
    drop(copied_nodes);
    let shift = Point2::new(node_pos.0 - min_pos.x, node_pos.1 - min_pos.y);

    let (copied_optical_nodes, copied_analyzer_nodes) =
        partition_cache(data.node_copy_cache.lock().iter().cloned());

    let mut analyzers = Vec::new();
    if paste_in_scenery {
        for analyzer_dto in &copied_analyzer_nodes {
            analyzers.push(copy_analyzer(&data, shift, &analyzer_dto.info));
        }
    }

    let mut document = data.document.lock();
    let root_ids: Vec<Uuid> = copied_optical_nodes.iter().map(OpticRef::uuid).collect();
    validate_relocated_references(document.scenery(), &root_ids, paste_group_id)?;

    let PastedNodes {
        grouped_node_infos,
        grouped_connect_info,
        node_id_link,
    } = insert_copied_nodes(
        document.scenery_mut(),
        paste_group_id,
        shift,
        &copied_optical_nodes,
    )?;

    let mut amplifier_state_inverses = propagate_amplifier_state(&mut document, &node_id_link);

    // Build GraphDelta::Composite for top-level pasted optical nodes and their connections
    let mut graph_deltas = Vec::new();
    if let Some(infos) = grouped_node_infos.get(&paste_group_id) {
        for info in infos {
            if let Ok((node_ref, _)) = document.scenery().node_recursive(info.uuid()) {
                graph_deltas.push(GraphDelta::NodeAdded {
                    group_id: paste_group_id,
                    node_id: info.uuid(),
                    node: node_ref,
                });
            }
        }
    }
    if let Some(conns) = grouped_connect_info.get(&paste_group_id) {
        for c in conns {
            graph_deltas.push(GraphDelta::NodesConnected {
                group_id: paste_group_id,
                connection: ConnectionInfo {
                    src_id: c.src_uuid(),
                    src_port: c.src_port().to_string(),
                    target_id: c.target_uuid(),
                    target_port: c.target_port().to_string(),
                    distance: meter!(c.distance()),
                },
                displaced_port_mappings: Vec::new(),
            });
        }
    }

    // Combine topological delta with analyzer removals and amplifier states into a single undo step
    let mut undo_commands = Vec::new();
    if !graph_deltas.is_empty() {
        undo_commands.push(Command::UndoGraph(Box::new(GraphDelta::Composite(
            graph_deltas,
        ))));
    }
    for analyzer in &analyzers {
        undo_commands.push(Command::RemoveAnalyzer(analyzer.clone()));
    }
    undo_commands.append(&mut amplifier_state_inverses);

    if !undo_commands.is_empty() {
        data.push_undo(Command::from_vec(undo_commands).expect("undo_commands is not empty"));
    }

    drop(document);

    Ok(Json(PasteNodesResponse {
        pasted_nodes: grouped_node_infos,
        pasted_analyzers: analyzers,
        pasted_connections: grouped_connect_info,
    }))
}

/// Replays every pasted group's own port map onto its freshly-created copy.
fn reconfigure_ports(
    scenery: &mut NodeGroup,
    grouped_node_refs: &[(Uuid, Vec<OpticRef>, bool)],
    input_port_maps: &HashMap<Uuid, PortMap>,
    output_port_maps: &HashMap<Uuid, PortMap>,
    node_id_link: &HashMap<Uuid, Uuid>,
    grouped_node_infos: &mut HashMap<Uuid, Vec<NodeInfo>>,
) -> Result<(), BackEndErrorResponse> {
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

    let new_id = document.add_analyzer_with_position(
        analyzer.analyzer_type().clone(),
        Some((new_pos.x, new_pos.y)),
    );

    let new_info = document.analyzers().get(&new_id).cloned().unwrap();
    drop(document);
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
        let node_id = node.uuid();

        let group_nodes_opt = {
            node.as_any().downcast_ref::<NodeGroup>().map(|group| {
                input_port_maps.insert(node_id, group.graph().port_map(&PortType::Input).clone());
                output_port_maps.insert(node_id, group.graph().port_map(&PortType::Output).clone());
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
    let (mut new_node_ref, old_node_id) = copy_from_optic_ref(scenery, optic_ref)?;

    let new_pos = get_shifted_pos_of_ref(optic_ref, shift);

    new_node_ref
        .node_attr_mut()
        .set_gui_position(Some(Point2::new(new_pos.0, new_pos.1)));

    node_id_link.insert(old_node_id, new_node_ref.uuid());

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
        let old_node_id = optic_ref.node_attr().uuid();
        let reference_uuid_opt = optic_ref
            .node_attr()
            .properties()
            .get("reference id")
            .ok()
            .and_then(|p| match p {
                Proptype::Uuid(id) => Some(*id),
                _ => None,
            });

        let node_type = optic_ref.node_type().to_string();
        let node_attr_clone = optic_ref.node_attr().clone();

        (old_node_id, reference_uuid_opt, node_type, node_attr_clone)
    };

    let referenced_node_opt = if let Some(ref_uuid) = reference_uuid_opt {
        Some(scenery.node_recursive(ref_uuid)?.0)
    } else {
        None
    };

    let mut new_node_ref = create_node_ref(&node_type)?;
    if let Some(referenced_node) = referenced_node_opt {
        if let Some(ref_node) = new_node_ref.as_any_mut().downcast_mut::<NodeReference>() {
            ref_node.assign_reference(&referenced_node)?;
        } else {
            return Err(OpossumError::Other("Cannot cast to reference node".into()).into());
        }
    }

    new_node_ref
        .node_attr_mut()
        .replace_from_node_attr(&node_attr_clone);

    Ok((new_node_ref, old_node_id))
}

fn get_shifted_pos_of_ref(optic_ref: &OpticRef, shift: Point2<f64>) -> (f64, f64) {
    let old_pos = optic_ref.gui_position().unwrap_or_else(Point2::origin);
    (old_pos.x + shift.x, old_pos.y + shift.y)
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use opossum_core::meter;

    use super::*;
    use crate::{
        document::{redo_document, undo_document},
        operations::copy::post_copy_nodes,
    };

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
            .set_json((root_id, (500.0, 500.0)))
            .to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);

        let pasted_group_id = {
            let document = app_state.document.lock();
            document
                .scenery()
                .with_group_node(root_id, |g| {
                    g.nodes()
                        .iter()
                        .map(|n| n.uuid())
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

        let req = test::TestRequest::post()
            .uri("/paste_nodes")
            .set_json((root_id, (500.0, 500.0)))
            .to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);

        let pasted_ids: Vec<Uuid> = {
            let document = app_state.document.lock();
            document
                .scenery()
                .with_group_node(root_id, |g| {
                    g.nodes()
                        .iter()
                        .map(|n| n.uuid())
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
                    g.nodes().iter().map(|n| n.uuid()).find(|id| *id != node_a)
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
            .set_json((root_id, (500.0, 500.0)))
            .to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);

        let pasted_ids: Vec<Uuid> = {
            let document = app_state.document.lock();
            document
                .scenery()
                .with_group_node(root_id, |g| {
                    g.nodes()
                        .iter()
                        .map(|n| n.uuid())
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
            .set_json((root_id, (500.0, 500.0)))
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
                    g.nodes().iter().map(|n| n.uuid()).find(|id| *id != g1_id)
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
            .set_json((g_id, (0.0, 0.0)))
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
            .set_json((g2_id, (0.0, 0.0)))
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::BAD_REQUEST,
            "pasting a reference into a nested descendant of its target must be rejected"
        );
    }

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
            .set_json((dest_id, (0.0, 0.0)))
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "pasting a reference together with its own target as siblings must still be allowed"
        );
    }

    #[actix_web::test]
    async fn test_paste_preserves_node_properties() {
        use opossum_core::{nodes::Lens, properties::Proptype};

        const TEST_PROP: &str = "test_prop";

        let app_state = Data::new(AppState::default());
        let (root_id, lens_id) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let lens_id = document.scenery_mut().add_node(Lens::default()).unwrap();
            document
                .scenery_mut()
                .with_node_attr_mut(lens_id, |attr| {
                    attr.create_property(TEST_PROP, "test", Proptype::Bool(true))
                })
                .unwrap()
                .unwrap();
            let ids = (root_id, lens_id);
            drop(document);
            ids
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_copy_nodes)
                .service(post_paste_nodes),
        )
        .await;

        let mut nodes_to_copy = HashSet::new();
        nodes_to_copy.insert(lens_id);
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
            .set_json((root_id, (500.0, 500.0)))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let pasted: PasteNodesResponse = test::read_body_json(resp).await;

        let pasted_node = pasted
            .pasted_nodes
            .values()
            .flatten()
            .find(|info| info.uuid() != lens_id)
            .expect("a pasted duplicate must exist");
        assert_ne!(
            pasted_node.uuid(),
            lens_id,
            "the copy must get a fresh uuid"
        );

        let document = app_state.document.lock();
        let property = document
            .scenery()
            .with_node_attr(pasted_node.uuid(), |attr| {
                attr.get_property(TEST_PROP).cloned()
            })
            .unwrap()
            .unwrap();
        assert!(
            matches!(property, Proptype::Bool(true)),
            "the pasted node must keep the original's property value"
        );
    }

    #[actix_web::test]
    async fn test_paste_preserves_amplifier_state() {
        use opossum_core::{
            gain::{ConstGain, GainModel, PumpConfig, PumpSource},
            nodes::Lens,
        };

        let app_state = Data::new(AppState::default());
        let gain = GainModel::Const(ConstGain::new(2.5).unwrap());
        let pump = PumpSource::Const;
        let (root_id, lens_id, scenario_id) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let lens_id = document.scenery_mut().add_node(Lens::default()).unwrap();
            document.set_is_amplifier_node(lens_id, true);
            let scenario_id = document.add_pump_scenario("full power");
            let scenario = document.pump_scenario_mut(scenario_id).unwrap();
            scenario.set_gain_model(lens_id, gain);
            scenario.set_pump_source(lens_id, pump);
            let ids = (root_id, lens_id, scenario_id);
            drop(document);
            ids
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_copy_nodes)
                .service(post_paste_nodes)
                .service(undo_document),
        )
        .await;

        let mut nodes_to_copy = HashSet::new();
        nodes_to_copy.insert(lens_id);
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
            .set_json((root_id, (500.0, 500.0)))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let pasted: PasteNodesResponse = test::read_body_json(resp).await;

        let pasted_id = pasted
            .pasted_nodes
            .values()
            .flatten()
            .find(|info| info.uuid() != lens_id)
            .expect("a pasted duplicate must exist")
            .uuid();

        {
            let document = app_state.document.lock();
            assert!(
                document.is_amplifier_node(pasted_id),
                "the copy of an amplifier must itself be an amplifier candidate"
            );
            assert_eq!(
                document
                    .pump_scenario(scenario_id)
                    .unwrap()
                    .config(pasted_id),
                PumpConfig::new(gain, pump),
                "the copy must carry the original's whole configuration into the same scenario"
            );
        }

        let req = test::TestRequest::post().uri("/undo").to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);

        let document = app_state.document.lock();
        assert!(
            !document.is_amplifier_node(pasted_id),
            "undo must remove the copy's candidacy along with the node itself"
        );
        assert_eq!(
            document
                .pump_scenario(scenario_id)
                .unwrap()
                .config(pasted_id),
            PumpConfig::default(),
            "undo must remove the copy's scenario entry along with the node itself"
        );
        assert!(
            document.is_amplifier_node(lens_id),
            "undo must not disturb the original's candidacy"
        );
        assert_eq!(
            document.pump_scenario(scenario_id).unwrap().config(lens_id),
            PumpConfig::new(gain, pump),
            "undo must not disturb the original's configuration"
        );
    }
}
