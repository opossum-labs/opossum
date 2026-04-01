use std::{collections::HashSet, fs, path::PathBuf};

use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use futures_util::StreamExt;
use opossum_core::{
    opm_document::AnalyzerInfo,
    prelude::{AnalyzerType, PortMap, PortType},
    types::api_types::{ConnectInfo, NewAnalyzerInfo, NewNode, NewRefNode, NodeInfo},
};
use uuid::Uuid;

use crate::{
    OPOSSUM_UI_LOGS,
    api::{self, eval_action_run},
    components::scenery_editor::{
        NodeType,
        constants::{
            HEADER_HEIGHT, MIN_NODE_DISTANCE_RADIUS, NODE_PLACEMENT_MAX_ITERATIONS, NODE_WIDTH,
        },
        graph_editor::graph_workspace::{
            GraphsWorkspaceState, WorkSpaceSignalHandlers,
            workspace_action::GraphsWorkspaceAction,
            workspace_state::{GraphInfo, optimize_layout_and_sync},
        },
        node::MIN_NODE_BODY_HEIGHT,
    },
};
#[allow(clippy::large_types_passed_by_value)]
#[allow(clippy::too_many_lines)]
pub fn use_workspace_processor(
    workspace: ReadSignal<GraphsWorkspaceState>,
    root_graph_id: Memo<Uuid>,
    workspace_handlers: WorkSpaceSignalHandlers,
    set_file_path_handler: EventHandler<Option<PathBuf>>,
) -> Coroutine<GraphsWorkspaceAction> {
    use_coroutine(move |mut rx: UnboundedReceiver<GraphsWorkspaceAction>| {
        async move {
            // This loop runs forever in the background, waiting for actions.
            while let Some(action) = rx.next().await {
                match action {
                    GraphsWorkspaceAction::LoadFromFile(path) => {
                        process_load_from_file(
                            path,
                            root_graph_id,
                            workspace_handlers,
                            set_file_path_handler,
                        )
                        .await;
                    }
                    GraphsWorkspaceAction::SaveToFile(path) => {
                        process_save_root_scenery_to_file(
                            path,
                            set_file_path_handler,
                            workspace_handlers,
                            root_graph_id(),
                        )
                        .await;
                    }
                    GraphsWorkspaceAction::DeleteRootScenery => {
                        process_delete_root_scenery(workspace_handlers, set_file_path_handler)
                            .await;
                    }
                    GraphsWorkspaceAction::AddRootSceneryTab { name } => {
                        process_add_root_scenery_tab(workspace_handlers, name).await;
                    }
                    GraphsWorkspaceAction::AddOpticNode {
                        node_type,
                        graph_id,
                    } => {
                        process_add_optic_node(&node_type, workspace, workspace_handlers, graph_id)
                            .await;
                    }
                    GraphsWorkspaceAction::AddOpticReference {
                        new_ref_node,
                        graph_id,
                    } => {
                        process_add_reference_node(new_ref_node, workspace_handlers, graph_id)
                            .await;
                    }
                    GraphsWorkspaceAction::AddAnalyzer {
                        analyzer_type,
                        graph_id,
                    } => {
                        process_add_analyzer(
                            analyzer_type,
                            workspace,
                            workspace_handlers,
                            graph_id,
                        )
                        .await;
                    }
                    GraphsWorkspaceAction::OptimizeLayout { graph_id } => {
                        process_optimize_layout(workspace, workspace_handlers, graph_id).await;
                    }
                    GraphsWorkspaceAction::CenterGraph {
                        graph_id,
                        save_changes,
                    } => workspace_handlers.view.center_graph(graph_id, save_changes),
                    GraphsWorkspaceAction::ZoomToFit {
                        graph_id,
                        save_changes,
                    } => workspace_handlers.view.zoom_to_fit(graph_id, save_changes),
                    GraphsWorkspaceAction::UpdateEdges {
                        connections,
                        graph_id,
                    } => workspace_handlers.edges.update_edges(connections, graph_id),
                    GraphsWorkspaceAction::InvertNode {
                        inverted,
                        graph_id,
                        node_id,
                    } => workspace_handlers
                        .nodes
                        .invert_node(node_id, inverted, graph_id),
                    GraphsWorkspaceAction::SetNodeName {
                        name,
                        graph_id,
                        node_id,
                        needs_saving,
                    } => workspace_handlers.nodes.set_node_name(
                        name,
                        node_id,
                        graph_id,
                        needs_saving,
                    ),
                    GraphsWorkspaceAction::UpdateEdge {
                        connection,
                        graph_id,
                    } => {
                        process_update_edge(connection, workspace_handlers, graph_id).await;
                    }
                    GraphsWorkspaceAction::DeleteEdge {
                        connection,
                        graph_id,
                    } => {
                        process_delete_edge(connection, workspace_handlers, graph_id).await;
                    }
                    GraphsWorkspaceAction::CopyNodes { nodes } => {
                        process_copy_nodes(nodes).await;
                    }
                    GraphsWorkspaceAction::PasteNode { pos, graph_id } => {
                        process_paste_node(pos, workspace_handlers, graph_id).await;
                    }
                    GraphsWorkspaceAction::SyncNodePosition { node_id, pos } => {
                        eval_action_run(
                            api::update_gui_position(node_id, pos).await,
                            Some(move |_| {
                                workspace_handlers.workspace.set_needs_saving(true);
                            }),
                        );
                    }
                    GraphsWorkspaceAction::AddEdge { new_edge, graph_id } => {
                        process_add_edge(new_edge, workspace_handlers, graph_id).await;
                    }
                    GraphsWorkspaceAction::DeleteNode { node_id, graph_id } => {
                        process_delete_node(node_id, workspace, workspace_handlers, graph_id).await;
                    }
                    GraphsWorkspaceAction::OpenGroupTab {
                        group_id,
                        group_name,
                    } => {
                        let group_tab_already_open =
                            workspace.read().tabs.read().contains_key(&group_id);
                        if group_tab_already_open {
                            workspace_handlers.workspace.set_active_tab(group_id);
                        } else {
                            process_open_group_tab(
                                group_id,
                                group_name,
                                workspace_handlers,
                                root_graph_id,
                            )
                            .await;
                        }
                    }
                    GraphsWorkspaceAction::ConvertToGroup { nodes, graph_id } => {
                        process_convert_nodes_to_group(nodes, graph_id, workspace_handlers).await;
                    }
                    GraphsWorkspaceAction::DropNodesIntoGroup {
                        nodes,
                        from_graph_id,
                        to_graph_id,
                    } => {
                        process_drop_nodes_into_group(
                            nodes,
                            from_graph_id,
                            to_graph_id,
                            workspace_handlers,
                            root_graph_id,
                        )
                        .await;
                    }
                    GraphsWorkspaceAction::MapNodePort {
                        port_type,
                        group_port_name,
                        mapped_node_port_name,
                        mapped_node_id,
                        group_id,
                    } => {
                        process_add_port_map(
                            port_type,
                            group_port_name,
                            mapped_node_port_name,
                            mapped_node_id,
                            group_id,
                            workspace_handlers,
                        )
                        .await;
                    }
                    GraphsWorkspaceAction::RemovePortMap {
                        group_id,
                        group_port_name,
                        port_type,
                    } => {
                        process_remove_port_map(
                            group_id,
                            group_port_name,
                            port_type,
                            workspace_handlers,
                        )
                        .await;
                    }
                    GraphsWorkspaceAction::SetDragStatus(drag_status) => workspace_handlers.workspace.set_drag_status(drag_status),
                    GraphsWorkspaceAction::SetDropInGroup(droppable_group) => workspace_handlers.workspace.set_drop_in_group(droppable_group),
                    GraphsWorkspaceAction::SetSelectionBox(selection_box) => workspace_handlers.workspace.set_selection_box(selection_box),
                }
            }
        }
    })
}

#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_add_edge(
    connect_info: ConnectInfo,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
) {
    eval_action_run(
        api::post_add_connection(connect_info, graph_id).await,
        Some(move |ci| {
            ws_handler.edges.add_edge(ci, graph_id);
        }),
    );
}

#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_delete_node(
    node_id: Uuid,
    workspace: ReadSignal<GraphsWorkspaceState>,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
) {
    let node_type_to_delete = {
        let graph = workspace.read().tabs.read().get(&graph_id).copied();

        let Some(graph) = graph else {
            OPOSSUM_UI_LOGS.write().add_log(&format!(
                "No graph with id '{}' found",
                graph_id.as_simple()
            ));
            return;
        };

        graph.read().graph_store.read().get_node_type(node_id)
    };
    if let Some(node_type) = node_type_to_delete {
        match node_type {
            NodeType::Optical(_) => {
                eval_action_run(
                    api::delete_node(node_id).await,
                    Some(move |deleted_ids| {
                        ws_handler.nodes.remove_nodes(deleted_ids, graph_id);
                    }),
                );
            }
            NodeType::Analyzer(_) => {
                eval_action_run(
                    api::delete_analyzer(node_id).await,
                    Some(move |deleted_id| {
                        ws_handler.nodes.remove_nodes(vec![deleted_id], graph_id);
                    }),
                );
            }
        }
    } else {
        OPOSSUM_UI_LOGS
            .write()
            .add_log("Node could not be deleted, as uuid was not found");
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_paste_node(
    pos: Point2D<f64>,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
) {
    match api::post_paste_nodes(graph_id, pos).await {
        Ok((optical_nodes, analyzer_nodes, edges)) => {
            let mut pasted_groups = Vec::<Uuid>::new();
            for (graph_id, n) in &optical_nodes {
                for node in n {
                    ws_handler.nodes.add_optical_node(node.clone(), *graph_id);
                    if node.node_type() == "group" {
                        pasted_groups.push(node.uuid());
                    }
                }
            }
            for a in &analyzer_nodes {
                let analyzer_id = a.id();
                ws_handler.nodes.add_analyzer_node(
                    NewAnalyzerInfo::from(a.clone()),
                    analyzer_id,
                    graph_id,
                );
            }

            for group_id in pasted_groups {
                eval_action_run(
                    api::get_port_maps_of_group(group_id).await,
                    Some(
                        move |(input_port_maps, output_port_maps): (PortMap, PortMap)| {
                            for (group_port_name, (mapped_node_id, mapped_node_port_name)) in
                                &input_port_maps
                            {
                                ws_handler.workspace.add_port_map(
                                    group_id,
                                    group_port_name.clone(),
                                    mapped_node_port_name.clone(),
                                    *mapped_node_id,
                                );
                            }
                            for (group_port_name, (mapped_node_id, mapped_node_port_name)) in
                                &output_port_maps
                            {
                                ws_handler.workspace.add_port_map(
                                    group_id,
                                    group_port_name.clone(),
                                    mapped_node_port_name.clone(),
                                    *mapped_node_id,
                                );
                            }
                            // println!("input_ports{:?}", input_ports);
                            // ws_handler
                            //     .nodes
                            //     .update_group_ports(input_ports, output_ports, group);
                        },
                    ),
                );
                eval_action_run(
                    api::get_ports_of_group(group_id).await,
                    Some(move |(input_ports, output_ports)| {
                        ws_handler
                            .nodes
                            .update_group_ports(input_ports, output_ports, group_id);
                    }),
                );
            }

            for (graph_id, edges) in &edges {
                for edge in edges {
                    ws_handler.edges.add_edge(edge.clone(), *graph_id);
                }
            }
        }
        Err(e) => {
            OPOSSUM_UI_LOGS
                .write()
                .add_log(&format!("Error while pasting node/s: {e}"));
        }
    }
    // Some(
    //     move |(optical_nodes, analyzer_nodes, edges): (
    //         HashMap<Uuid, Vec<NodeInfo>>,
    //         Vec<AnalyzerInfo>,
    //         HashMap<Uuid, Vec<ConnectInfo>>,
    //     )| {
    //         for (graph_id, n) in &optical_nodes {
    //             for node in n {
    //                 ws_handler.nodes.add_optical_node(node.clone(), *graph_id);
    //             }
    //         }
    //         for a in &analyzer_nodes {
    //             let analyzer_id = a.id();
    //             ws_handler.nodes.add_analyzer_node(
    //                 NewAnalyzerInfo::from(a.clone()),
    //                 analyzer_id,
    //                 graph_id,
    //             );
    //         }
    //         for (graph_id, edges) in &edges {
    //             for edge in edges {
    //                 ws_handler.edges.add_edge(edge.clone(), *graph_id);
    //             }
    //         }
    //     },
    // ),
}

#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_copy_nodes(nodes: HashSet<Uuid>) {
    eval_action_run(api::post_copy_nodes(nodes).await, None::<fn(String)>);
}

#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_delete_edge(
    connect_info: ConnectInfo,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
) {
    eval_action_run(
        api::delete_connection(connect_info.clone(), graph_id).await,
        Some(move |_| ws_handler.edges.delete_edge(connect_info, graph_id)),
    );
}

#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_update_edge(
    connect_info: ConnectInfo,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
) {
    eval_action_run(
        api::update_distance(connect_info, graph_id).await,
        Some(move |ci: ConnectInfo| ws_handler.edges.update_edge(ci, graph_id)),
    );
}

#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_optimize_layout(
    workspace: ReadSignal<GraphsWorkspaceState>,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
) {
    let Some(edges) = workspace
        .peek()
        .tabs
        .peek()
        .get(&graph_id)
        .map(|g| g.read().graph_store.read().edges().read().clone())
    else {
        OPOSSUM_UI_LOGS.write().add_log(&format!(
            "No graph with id '{}' found",
            graph_id.as_simple()
        ));
        return;
    };

    eval_action_run(
        optimize_layout_and_sync(edges).await,
        Some(move |new_positions| {
            ws_handler
                .nodes
                .update_node_positions(new_positions, graph_id);
        }),
    );
}

#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_add_analyzer(
    analyzer_type: AnalyzerType,
    workspace: ReadSignal<GraphsWorkspaceState>,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
) {
    // ----- READ PHASE -----
    let new_analyzer_info = {
        let graph = workspace.peek().tabs.peek().get(&graph_id).copied();

        let Some(graph) = graph else {
            OPOSSUM_UI_LOGS.write().add_log(&format!(
                "No graph with id '{}' found",
                graph_id.as_simple()
            ));
            return;
        };

        let editor_state = *graph.peek().editor_state.peek();
        let graph_store = *graph.peek().graph_store.peek();

        let zoom = *editor_state.zoom.peek();
        let shift = *editor_state.shift.peek();
        let center = workspace.read().get_view_port_center();

        let proposed_pos = ((center.x - shift.x) / zoom, (center.y - shift.y) / zoom);

        let existing_positions: Vec<_> = graph_store.nodes()()
            .values()
            .map(|n| (n.pos().x, n.pos().y))
            .collect();

        let final_pos = find_suitable_element_position(proposed_pos, &existing_positions);

        NewAnalyzerInfo::new(analyzer_type, final_pos)
    };

    eval_action_run(
        api::post_add_analyzer(new_analyzer_info.clone()).await,
        Some(move |analyzer_id| {
            ws_handler
                .nodes
                .add_analyzer_node(new_analyzer_info, analyzer_id, graph_id);
        }),
    );
}

#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_add_reference_node(
    new_ref_node: NewRefNode,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
) {
    let result = api::post_add_ref_node(new_ref_node, graph_id).await;
    eval_action_run(
        result,
        Some(move |node_info| ws_handler.nodes.add_reference_node(node_info, graph_id)),
    );
}

#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_add_optic_node(
    new_node_type_string: &str,
    workspace: ReadSignal<GraphsWorkspaceState>,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
) {
    // ----- READ PHASE -----
    let new_node_info = {
        let graph = workspace.peek().tabs.peek().get(&graph_id).copied();

        let Some(graph) = graph else {
            OPOSSUM_UI_LOGS.write().add_log(&format!(
                "No graph with id '{}' found",
                graph_id.as_simple()
            ));
            return;
        };

        let editor_state = *graph.peek().editor_state.peek();
        let graph_store = *graph.peek().graph_store.peek();

        let zoom = *editor_state.zoom.peek();

        let shift = *editor_state.shift.peek();
        let center = workspace.read().get_view_port_center();
        let proposed_pos = (
            (center.x - shift.x - NODE_WIDTH / 2.) / zoom,
            (center.y - shift.y - f64::midpoint(MIN_NODE_BODY_HEIGHT, HEADER_HEIGHT)) / zoom,
        );

        let existing_positions: Vec<_> = graph_store.nodes()()
            .values()
            .map(|n| (n.pos().x, n.pos().y))
            .collect();

        let final_pos = find_suitable_element_position(proposed_pos, &existing_positions);

        NewNode::new(new_node_type_string.to_lowercase(), final_pos)
    };

    // ----- ASYNC PHASE -----
    let result = api::post_add_node(new_node_info, graph_id).await;

    // ----- WRITE PHASE -----
    eval_action_run(
        result,
        Some(move |node_info| {
            ws_handler.nodes.add_optical_node(node_info, graph_id);
        }),
    );
}

fn find_suitable_element_position(
    proposed_position: (f64, f64),
    existing_element_positions: &[(f64, f64)],
) -> (f64, f64) {
    let mut final_position = proposed_position;
    let min_dist_squared = MIN_NODE_DISTANCE_RADIUS.powi(2);
    for _ in 0..NODE_PLACEMENT_MAX_ITERATIONS {
        let has_collision = existing_element_positions.iter().any(|&(pos_x, pos_y)| {
            let dist_x = final_position.0 - pos_x;
            let dist_y = final_position.1 - pos_y;
            let dist_sq = dist_x.mul_add(dist_x, dist_y * dist_y);
            dist_sq < min_dist_squared
        });
        if has_collision {
            final_position.0 += MIN_NODE_DISTANCE_RADIUS;
            final_position.1 += MIN_NODE_DISTANCE_RADIUS;
        } else {
            return final_position;
        }
    }
    final_position // fallback: return last position after reaching max iterations
}
#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_remove_port_map(
    group_id: Uuid,
    group_port_name: String,
    port_type: PortType,
    ws_handler: WorkSpaceSignalHandlers,
) {
    match api::remove_port_map(group_port_name.clone(), group_id, port_type).await {
        Ok((removed_port, removed_connections, parent_group_id)) => {
            for edge in &removed_connections {
                ws_handler.edges.delete_edge(edge.clone(), parent_group_id);
            }
            if removed_port {
                ws_handler
                    .workspace
                    .remove_port_map(group_id, group_port_name.clone());
                ws_handler
                    .nodes
                    .remove_group_port(group_port_name, group_id, port_type);
            } else {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log("Could not remove port mapping!");
            }
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }
}
#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_add_port_map(
    port_type: PortType,
    group_port_name: String,
    mapped_node_port_name: String,
    mapped_node_id: Uuid,
    group_id: Uuid,
    ws_handler: WorkSpaceSignalHandlers,
) {
    match api::add_port_map(
        port_type,
        group_port_name.clone(),
        mapped_node_port_name.clone(),
        mapped_node_id,
        group_id,
    )
    .await
    {
        Ok((input_ports, output_ports)) => {
            ws_handler.workspace.add_port_map(
                group_id,
                group_port_name,
                mapped_node_port_name,
                mapped_node_id,
            );
            ws_handler
                .nodes
                .update_group_ports(input_ports, output_ports, group_id);
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }
}

#[allow(clippy::large_types_passed_by_value)]
#[allow(clippy::future_not_send)]
async fn process_drop_nodes_into_group(
    nodes: Vec<Uuid>,
    from_group_id: Uuid,
    drop_group_id: Uuid,
    ws_handler: WorkSpaceSignalHandlers,
    root_scenery_id: Memo<Uuid>,
) {
    match api::drop_nodes_into_group(nodes.clone(), from_group_id, drop_group_id).await {
        Ok(_) => {
            //remove nodes that have been dropped into a group from graph
            ws_handler.nodes.remove_nodes(nodes, from_group_id);

            process_fill_graph_of_group(root_scenery_id, drop_group_id, ws_handler).await;
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }

    ws_handler.nodes.remove_droppable_group();
}

#[allow(clippy::large_types_passed_by_value)]
#[allow(clippy::future_not_send)]
async fn process_convert_nodes_to_group(
    nodes: Vec<Uuid>,
    current_group_id: Uuid,
    ws_handler: WorkSpaceSignalHandlers,
) {
    match api::convert_nodes_to_group(nodes.clone(), current_group_id).await {
        Ok((new_group_info, port_mapping)) => {
            //remove nodes that have been converted to a group from graph
            ws_handler.nodes.remove_nodes(nodes, current_group_id);

            //add new group node
            ws_handler
                .nodes
                .add_optical_node(new_group_info, current_group_id);

            //connect group node
            for edge in port_mapping {
                ws_handler.edges.add_edge(edge, current_group_id);
            }
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }
}

#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_open_group_tab(
    group_id: Uuid,
    group_name: String,
    ws_handler: WorkSpaceSignalHandlers,
    root_scenery_id: Memo<Uuid>,
) {
    eval_action_run(
        api::get_group_hierarchy(group_id).await,
        Some(move |group_hierarchy: Vec<(Uuid, String)>| {
            let graph_info = GraphInfo {
                name: group_name,
                id: group_id,
                hierarchy: group_hierarchy,
            };
            ws_handler.workspace.add_new_group_tab(graph_info);
        }),
    );
    process_fill_graph_of_group(root_scenery_id, group_id, ws_handler).await;
}

#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_fill_graph_of_group(
    root_scenery_id: Memo<Uuid>,
    group_id: Uuid,
    ws_handler: WorkSpaceSignalHandlers,
) {
    eval_action_run(
        api::get_nodes(group_id).await,
        Some(move |nodes: Vec<NodeInfo>| ws_handler.nodes.add_group_nodes(group_id, nodes)),
    );

    eval_action_run(
        api::get_port_maps_of_group(group_id).await,
        Some(move |(input_ports, output_ports): (PortMap, PortMap)| {
            for (group_port_name, (mapped_node_id, mapped_node_port_name)) in &input_ports {
                ws_handler.workspace.add_port_map(
                    group_id,
                    group_port_name.clone(),
                    mapped_node_port_name.clone(),
                    *mapped_node_id,
                );
            }
            for (group_port_name, (mapped_node_id, mapped_node_port_name)) in &output_ports {
                ws_handler.workspace.add_port_map(
                    group_id,
                    group_port_name.clone(),
                    mapped_node_port_name.clone(),
                    *mapped_node_id,
                );
            }
        }),
    );

    eval_action_run(
        api::get_connections(group_id).await,
        Some(move |connect_infos: Vec<ConnectInfo>| {
            ws_handler.edges.add_group_edges(group_id, connect_infos);
        }),
    );

    if *root_scenery_id.read() == group_id {
        eval_action_run(
            api::get_analyzers().await,
            Some(move |analyzers: Vec<AnalyzerInfo>| {
                ws_handler.nodes.add_group_analyzers(group_id, analyzers);
            }),
        );
    }
    ws_handler.view.center_graph(group_id, false);
}
#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_load_from_file(
    path: PathBuf,
    scenery_id_sig: Memo<Uuid>,
    ws_handler: WorkSpaceSignalHandlers,
    set_file_path_handler: EventHandler<Option<PathBuf>>,
) {
    process_delete_root_scenery(ws_handler, set_file_path_handler).await;
    let opm_string = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            OPOSSUM_UI_LOGS.write().add_log(&e.to_string());
            return;
        }
    };
    match api::post_opm_file(opm_string).await {
        Ok(name) => {
            process_add_root_scenery_tab(ws_handler, name).await;
            set_file_path_handler.call(Some(path));
            let scenery_id = *scenery_id_sig.read();
            process_fill_graph_of_group(scenery_id_sig, scenery_id, ws_handler).await;
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }
}

#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_delete_root_scenery(
    workspace_handlers: WorkSpaceSignalHandlers,
    set_file_path_handler: EventHandler<Option<PathBuf>>,
) {
    eval_action_run(
        api::delete_scenery().await,
        Some(move |_| {
            workspace_handlers.workspace.clear_workspace();
            set_file_path_handler.call(None);
        }),
    );
}

#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_save_root_scenery_to_file(
    path: PathBuf,
    set_file_path_handler: EventHandler<Option<PathBuf>>,
    ws_handler: WorkSpaceSignalHandlers,
    root_id: Uuid,
) {
    if let Some(f_stem) = path.file_stem()
        && let Some(fname) = f_stem.to_str()
    {
        process_rename_root_scenery(ws_handler, fname.to_string(), root_id, false).await;
        eval_action_run(
            api::get_opm_file().await,
            Some(move |opm_string| {
                if let Err(err_str) = fs::write(&path, opm_string) {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str.to_string());
                } else {
                    set_file_path_handler.call(Some(path));
                    ws_handler.workspace.set_needs_saving(false);
                }
            }),
        );
    }
}

#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_add_root_scenery_tab(ws_handler: WorkSpaceSignalHandlers, name: String) {
    match api::get_scenery_uuid().await {
        Ok(id) => {
            ws_handler.workspace.set_root_scenery_id(id);
            ws_handler.workspace.add_new_group_tab(GraphInfo {
                name: name.clone(),
                id,
                hierarchy: vec![(id, name.clone())],
            });
            process_rename_root_scenery(ws_handler, name, id, false).await;
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }
}

#[allow(clippy::future_not_send)]
#[allow(clippy::large_types_passed_by_value)]
async fn process_rename_root_scenery(
    ws_handler: WorkSpaceSignalHandlers,
    name: String,
    root_id: Uuid,
    needs_saving: bool,
) {
    eval_action_run(
        api::update_node_name(root_id, name.clone()).await,
        Some(move |_| {
            ws_handler
                .nodes
                .set_node_name(name, root_id, root_id, needs_saving);
        }),
    );
}
