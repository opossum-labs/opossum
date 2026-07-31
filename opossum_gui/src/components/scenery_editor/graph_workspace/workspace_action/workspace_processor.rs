#![allow(clippy::future_not_send)]
#![allow(clippy::large_types_passed_by_value)]
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
};

use dioxus::{
    html::geometry::euclid::default::{Point2D, Rect, Size2D},
    prelude::*,
};
use futures_util::StreamExt;
use opossum_core::{
    prelude::{AnalyzerType, PortType},
    types::api_types::{
        AnalyzerItemDto, ConnectInfo, CutResult, DeleteNodeResponse, DocumentChange,
        NewAnalyzerInfo, NewNode, NewRefNode, NodeEditorPanel, NodeInfo, NodePortsResponse,
        PasteNodesResponse, PortMappingsResponse, PositionUpdate, UndoRedoResponse,
        UpdateConnectionRequest, Viewport,
    },
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    LAST_AUTO_JUMPED_GRAPH, LAST_AUTO_SELECTED_NODE, NODE_DETAILS_REFRESH, OPOSSUM_UI_LOGS,
    PENDING_PANEL_OPEN,
    api::{self, delete_document, eval_action_run},
    components::scenery_editor::{
        DragStatus, NodeType,
        constants::{
            HEADER_HEIGHT, MIN_NODE_DISTANCE_RADIUS, NODE_PLACEMENT_MAX_ITERATIONS, NODE_WIDTH,
        },
        graph_workspace::{
            EditorStateStoreExt, GraphStateStoreExt, GraphStoreStoreExt, GraphsWorkspaceState,
            GraphsWorkspaceStateStoreExt, GraphsWorkspaceStateStoreImplExt,
            WorkSpaceSignalHandlers,
            workspace_action::GraphsWorkspaceAction,
            workspace_state::{GraphInfo, optimize_layout},
        },
        node::MIN_NODE_BODY_HEIGHT,
    },
};
#[allow(clippy::too_many_lines)]
pub fn use_workspace_processor(
    workspace: ReadStore<GraphsWorkspaceState>,
    root_graph_id: Memo<Uuid>,
    workspace_handlers: WorkSpaceSignalHandlers,
    set_file_path_handler: EventHandler<Option<PathBuf>>,
) -> Coroutine<GraphsWorkspaceAction> {
    use_coroutine(move |mut rx: UnboundedReceiver<GraphsWorkspaceAction>| {
        async move {
            // Viewport of the active tab captured when a graph-pan drag started, so the completed pan can
            // be recorded as one undo step on release. `None` unless a graph pan is in progress.
            let mut pan_before: Option<Viewport> = None;
            // This loop runs forever in the background, waiting for actions.
            while let Some(action) = rx.next().await {
                // A document-mutating action pushes an undo entry on the backend (=> can_undo becomes
                // true, can_redo false). Capture that classification before `action` is consumed and
                // reflect it after processing, so the Edit menu's Undo/Redo enabled-state stays correct.
                let was_document_edit = is_document_edit_action(&action);
                match action {
                    GraphsWorkspaceAction::LoadFromFile(path) => {
                        process_load_from_file(
                            workspace,
                            path,
                            root_graph_id,
                            workspace_handlers,
                            set_file_path_handler,
                        )
                        .await;
                        // The backend clears its undo/redo history on every load; mirror that here.
                        *crate::UNDO_REDO_STATUS.write() = (false, false);
                    }
                    GraphsWorkspaceAction::SaveToFile(path) => {
                        process_save_root_scenery_to_file(
                            path,
                            set_file_path_handler,
                            workspace_handlers,
                            root_graph_id(),
                            workspace,
                        )
                        .await;
                    }
                    GraphsWorkspaceAction::DeleteRootScenery => {
                        process_delete_root_scenery(workspace_handlers, set_file_path_handler)
                            .await;
                        // The backend clears its undo/redo history on every reset; mirror that here.
                        *crate::UNDO_REDO_STATUS.write() = (false, false);
                    }
                    GraphsWorkspaceAction::AddRootSceneryTab { name } => {
                        process_add_root_scenery_tab(workspace, workspace_handlers, name).await;
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
                        record_undo,
                    } => {
                        let before = current_viewport(workspace, graph_id);
                        workspace_handlers.view.center_graph(graph_id, save_changes);
                        if record_undo
                            && let (Some(before), Some(after)) =
                                (before, current_viewport(workspace, graph_id))
                        {
                            push_viewport_change(before, after, false);
                        }
                    }
                    GraphsWorkspaceAction::ZoomToFit {
                        graph_id,
                        save_changes,
                        merge_into_previous_undo,
                    } => {
                        let before = current_viewport(workspace, graph_id);
                        workspace_handlers.view.zoom_to_fit(graph_id, save_changes);
                        if let (Some(before), Some(after)) =
                            (before, current_viewport(workspace, graph_id))
                        {
                            push_viewport_change(before, after, merge_into_previous_undo);
                        }
                    }
                    // GraphsWorkspaceAction::UpdateEdges {
                    //     connections,
                    //     graph_id,
                    // } => workspace_handlers.edges.update_edges(connections, graph_id),
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
                        workspace_handlers.workspace.set_nodes_cut(false);
                    }
                    GraphsWorkspaceAction::CutNodes { nodes } => {
                        process_copy_nodes(nodes).await;
                        workspace_handlers.workspace.set_nodes_cut(true);
                    }
                    GraphsWorkspaceAction::PasteNode { pos, graph_id } => {
                        let nodes_cut = *workspace.nodes_cut().read();
                        process_paste_nodes(
                            pos,
                            workspace_handlers,
                            graph_id,
                            nodes_cut,
                            root_graph_id,
                            workspace,
                        )
                        .await;
                    }
                    GraphsWorkspaceAction::SyncNodePositions { moves } => {
                        let updates = moves
                            .iter()
                            .map(|(uuid, is_optical, pos)| PositionUpdate {
                                uuid: *uuid,
                                is_optical: *is_optical,
                                gui_position: (pos.x, pos.y),
                            })
                            .collect();
                        eval_action_run(
                            api::patch_positions(updates).await,
                            Some(move |()| {
                                workspace_handlers.workspace.set_needs_saving(true);
                            }),
                        );
                    }
                    GraphsWorkspaceAction::AddEdge { new_edge, graph_id } => {
                        process_add_edge(new_edge, workspace_handlers, graph_id).await;
                    }
                    GraphsWorkspaceAction::DeleteNodes { node_ids, graph_id } => {
                        process_delete_nodes(node_ids, workspace, workspace_handlers, graph_id)
                            .await;
                    }
                    GraphsWorkspaceAction::OpenGroupTab {
                        group_id,
                        group_name,
                    } => {
                        // A tab's data can exist in `tabs()` without being visible yet (silently
                        // seeded by `ensure_group_tab` for a group that was never opened) - judge
                        // "already open" by tab bar visibility, not data existence, or a
                        // silently-seeded group would never actually open.
                        let group_tab_already_open =
                            workspace.tab_order().read().contains(&group_id);
                        if group_tab_already_open {
                            workspace_handlers.workspace.set_active_tab(group_id);
                        } else {
                            process_open_group_tab(
                                group_id,
                                group_name,
                                workspace_handlers,
                                root_graph_id.into(),
                                workspace, // <-- Hier übergeben wir workspace
                            )
                            .await;
                        }
                    }
                    GraphsWorkspaceAction::ConvertToGroup { nodes, graph_id } => {
                        process_convert_nodes_to_group(
                            nodes,
                            graph_id,
                            workspace_handlers,
                            workspace,
                        )
                        .await;
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
                            workspace, // <-- Hier übergeben wir workspace
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
                    GraphsWorkspaceAction::SetDragStatus(drag_status) => {
                        // A graph pan is a viewport gesture: remember the camera when it starts, and on
                        // release record the whole pan as one undo step. Node drags / selection use other
                        // DragStatus values and are left alone (their position edit is a separate step).
                        match drag_status {
                            DragStatus::Graph => {
                                pan_before =
                                    current_viewport(workspace, *workspace.active_tab().read());
                            }
                            DragStatus::None => {
                                if let Some(before) = pan_before.take()
                                    && let Some(after) =
                                        current_viewport(workspace, before.graph_id)
                                {
                                    push_viewport_change(before, after, false);
                                }
                            }
                            _ => {}
                        }
                        workspace_handlers.workspace.set_drag_status(drag_status);
                    }
                    GraphsWorkspaceAction::SetDropInGroup(droppable_group) => workspace_handlers
                        .workspace
                        .set_drop_in_group(droppable_group),
                    GraphsWorkspaceAction::SetSelectionBox(selection_box) => workspace_handlers
                        .workspace
                        .set_selection_box(selection_box),
                    GraphsWorkspaceAction::ClearSelectedNodes { graph_id } => {
                        workspace_handlers.workspace.clear_selected_nodes(graph_id);
                    }
                    GraphsWorkspaceAction::SetEdgeInCreation {
                        graph_id,
                        edge_in_creation,
                    } => workspace_handlers
                        .edges
                        .set_edge_in_creation(edge_in_creation, graph_id),
                    GraphsWorkspaceAction::ApplyDrag {
                        graph_id,
                        relative_shift,
                        current_zoom,
                        mouse_to_graph_shift,
                    } => workspace_handlers.workspace.apply_drag(
                        graph_id,
                        relative_shift,
                        current_zoom,
                        mouse_to_graph_shift,
                    ),
                    GraphsWorkspaceAction::NodeClick {
                        graph_id,
                        node_id,
                        is_optical_node,
                        z_index,
                        ctrl_pressed,
                    } => workspace_handlers.nodes.node_click(
                        graph_id,
                        node_id,
                        is_optical_node,
                        z_index,
                        ctrl_pressed,
                    ),
                    GraphsWorkspaceAction::SetNodeActive {
                        graph_id,
                        node_id,
                        is_optical_node,
                        z_index,
                    } => workspace_handlers.nodes.set_node_active(
                        graph_id,
                        node_id,
                        is_optical_node,
                        z_index,
                    ),
                    GraphsWorkspaceAction::RemoveFromNodeSelection { graph_id, node_id } => {
                        workspace_handlers
                            .nodes
                            .remove_from_node_selection(graph_id, node_id);
                    }
                    GraphsWorkspaceAction::AddToNodeSelection {
                        graph_id,
                        node_id,
                        is_optical,
                    } => workspace_handlers
                        .nodes
                        .add_to_node_selection(graph_id, node_id, is_optical),
                    GraphsWorkspaceAction::SetZoom { graph_id, zoom } => {
                        workspace_handlers.view.set_zoom(graph_id, zoom);
                    }
                    GraphsWorkspaceAction::SetShift { graph_id, shift } => {
                        workspace_handlers.view.set_shift(graph_id, shift);
                    }
                    GraphsWorkspaceAction::RemoveTabs(uuids) => {
                        workspace_handlers.workspace.remove_tabs(uuids);
                    }
                    GraphsWorkspaceAction::SetActiveTab(uuid) => {
                        workspace_handlers.workspace.set_active_tab(uuid);
                    }
                    GraphsWorkspaceAction::JumpToMappedPort {
                        mapped_node_id,
                        parent,
                    } => {
                        process_jump_to_mapped_port(
                            root_graph_id.into(),
                            workspace, // <-- workspace war hier schon drin
                            workspace_handlers,
                            mapped_node_id,
                            parent.0,
                            parent.1,
                        )
                        .await;
                    }
                    GraphsWorkspaceAction::GetEditorArea => {
                        process_get_editor_area(workspace, workspace_handlers).await;
                    }
                    GraphsWorkspaceAction::Undo => {
                        eval_action_run(
                            api::undo_document().await,
                            Some(move |r: UndoRedoResponse| {
                                handle_undo_redo_response(
                                    r,
                                    root_graph_id,
                                    workspace,
                                    workspace_handlers,
                                );
                            }),
                        );
                    }
                    GraphsWorkspaceAction::Redo => {
                        eval_action_run(
                            api::redo_document().await,
                            Some(move |r: UndoRedoResponse| {
                                handle_undo_redo_response(
                                    r,
                                    root_graph_id,
                                    workspace,
                                    workspace_handlers,
                                );
                            }),
                        );
                    }
                }
                if was_document_edit {
                    // The edit pushed an undo entry on the backend; reflect that so the Edit menu enables
                    // Undo and greys out Redo. (Viewport gestures and node-editor edits mark this at their
                    // own push points.)
                    *crate::UNDO_REDO_STATUS.write() = (true, false);
                }
            }
        }
    })
}

/// Reads `graph_id`'s current viewport (pan/zoom) from its `EditorState`, if that tab exists.
fn current_viewport(
    workspace: ReadStore<GraphsWorkspaceState>,
    graph_id: Uuid,
) -> Option<Viewport> {
    let editor_state = workspace.tabs().get(graph_id).map(|g| g.editor_state())?;
    let zoom = *editor_state.zoom().peek();
    let shift = *editor_state.shift().peek();
    Some(Viewport {
        graph_id,
        zoom,
        shift: (shift.x, shift.y),
    })
}

/// Records a discrete camera gesture (pan, center, zoom-to-fit) as an undo step, fire-and-forget.
/// Sent with `coalesce=false`, so it never merges with an adjacent *zoom* - each such gesture stays a
/// separate undo step. A no-op move (`before == after`, e.g. centering an already-centered graph) is
/// dropped entirely: the backend would discard it anyway, but the optimistic status write below must
/// not enable the Undo button for it either.
///
/// `merge_into_previous` folds this move into the immediately preceding edit's undo entry instead of
/// pushing its own - used for the zoom-to-fit Auto Layout runs right after re-positioning the nodes,
/// so a single undo reverts both.
fn push_viewport_change(before: Viewport, after: Viewport, merge_into_previous: bool) {
    if before == after {
        return;
    }
    // A camera move is undoable, so enable Undo / grey out Redo like any other edit.
    *crate::UNDO_REDO_STATUS.write() = (true, false);
    spawn(async move {
        let _ = api::post_viewport_change(before, after, false, merge_into_previous).await;
    });
}

/// Whether processing this action mutates the document (and therefore pushes an undo entry on the
/// backend). Used to keep [`crate::UNDO_REDO_STATUS`] correct after edits. Viewport gestures mark it at
/// their own push point ([`push_viewport_change`]); `InvertNode`/`SetNodeName` are GUI mirrors whose
/// real edit is marked in the node editor - so both are excluded here.
const fn is_document_edit_action(action: &GraphsWorkspaceAction) -> bool {
    matches!(
        action,
        GraphsWorkspaceAction::AddOpticNode { .. }
            | GraphsWorkspaceAction::AddOpticReference { .. }
            | GraphsWorkspaceAction::AddAnalyzer { .. }
            | GraphsWorkspaceAction::OptimizeLayout { .. }
            | GraphsWorkspaceAction::UpdateEdge { .. }
            | GraphsWorkspaceAction::DeleteEdge { .. }
            | GraphsWorkspaceAction::PasteNode { .. }
            | GraphsWorkspaceAction::AddEdge { .. }
            | GraphsWorkspaceAction::DeleteNodes { .. }
            | GraphsWorkspaceAction::ConvertToGroup { .. }
            | GraphsWorkspaceAction::DropNodesIntoGroup { .. }
            | GraphsWorkspaceAction::MapNodePort { .. }
            | GraphsWorkspaceAction::RemovePortMap { .. }
            | GraphsWorkspaceAction::SyncNodePositions { .. }
    )
}

/// Handles an undo/redo endpoint response: reflects the resulting Undo/Redo availability, marks the
/// document unsaved, and replays the returned changes onto the canvas. Shared by the `Undo` and `Redo`
/// action arms, which differ only in which endpoint they call.
fn handle_undo_redo_response(
    r: UndoRedoResponse,
    root_graph_id: Memo<Uuid>,
    workspace: ReadStore<GraphsWorkspaceState>,
    ws_handler: WorkSpaceSignalHandlers,
) {
    *crate::UNDO_REDO_STATUS.write() = (r.can_undo, r.can_redo);
    ws_handler.workspace.set_needs_saving(true);
    spawn(apply_document_changes(
        r.changes,
        root_graph_id,
        workspace,
        ws_handler,
    ));
}

/// Applies the `DocumentChange`s returned by an undo/redo, by replaying each one through the exact
/// same `WorkSpaceSignalHandlers` calls the corresponding *normal* action already uses - so undo/redo
/// updates the canvas precisely, without reloading the whole workspace.
///
/// `NodeDetailsChanged`/`AnalyzerChanged`/`NodePatched` (custom properties, isometry, alignment, port
/// config, analyzer settings) aren't mirrored in `GraphStore` at all - only the properties panel shows
/// them. Rather than growing this function to know every such field, those three arms instead bump
/// `NODE_DETAILS_REFRESH`, a signal the properties panel's own `use_resource` reads unconditionally so it
/// refetches even when the selected node's identity hasn't changed (see that signal's doc comment).
#[allow(clippy::too_many_lines)]
async fn apply_document_changes(
    changes: Vec<DocumentChange>,
    root_graph_id: Memo<Uuid>,
    workspace: ReadStore<GraphsWorkspaceState>,
    ws_handler: WorkSpaceSignalHandlers,
) {
    // Collected while applying the changes below, then resolved once at the end - see the
    // auto-select-and-open-panel handling after the loop.
    let mut panel_requests: Vec<(Uuid, Uuid, NodeEditorPanel)> = Vec::new();
    // Same idea for changes that don't have a specific node/panel to select but still mutated the
    // document, not just the GUI (node/edge/analyzer existence, connection distance, or a
    // GraphNeedsRefresh-reported structural/port-map cascade) - see the tab-jump handling after the
    // loop. Used only as the fallback path's candidate pool - see `primary_structural_graph_id`.
    let mut structural_change_graph_ids: Vec<Uuid> = Vec::new();
    // The one graph_id this batch's own data unambiguously names as "where the change actually
    // happened", if any - set (first match wins) from either an inherently single-tab change
    // (NodeAdded/NodeRemoved/EdgeAdded/EdgeRemoved/EdgeUpdated/AnalyzerAdded/AnalyzerRemoved, which
    // never have cascade ambiguity to begin with) or a `GraphNeedsRefresh { is_origin: true }` (the
    // backend-tagged origin of a port-map cascade - see `port_map_commands::describe`). Preferred
    // over the `LAST_AUTO_JUMPED_GRAPH` heuristic below whenever it's available, since it's carried
    // in the response itself rather than guessed client-side - see the tab-jump handling after the
    // loop.
    let mut primary_structural_graph_id: Option<Uuid> = None;

    for change in changes {
        match change {
            DocumentChange::NodeAdded { graph_id, node } => {
                ws_handler.nodes.add_optical_node(*node, graph_id);
                structural_change_graph_ids.push(graph_id);
                primary_structural_graph_id.get_or_insert(graph_id);
            }
            DocumentChange::NodeRemoved { graph_id, uuid } => {
                ws_handler.nodes.remove_nodes(vec![uuid], graph_id);
                structural_change_graph_ids.push(graph_id);
                primary_structural_graph_id.get_or_insert(graph_id);
            }
            DocumentChange::NodePatched {
                graph_id,
                uuid,
                name,
                inverted,
                gui_position,
                panel,
            } => {
                if let Some(name) = name {
                    // Mirror the fan-out a normal rename does: propagate to every node referencing it.
                    if let Ok(node_refs_grouped) = api::get_node_references(uuid).await {
                        let ref_name = format!("ref ({name})");
                        for (group_id, ref_ids) in &node_refs_grouped {
                            for ref_id in ref_ids {
                                let new_name = if uuid == *ref_id {
                                    name.clone()
                                } else {
                                    ref_name.clone()
                                };
                                ws_handler
                                    .nodes
                                    .set_node_name(new_name, *ref_id, *group_id, true);
                            }
                        }
                    } else {
                        ws_handler.nodes.set_node_name(name, uuid, graph_id, true);
                    }
                }
                if let Some(inverted) = inverted {
                    ws_handler.nodes.invert_node(uuid, inverted, graph_id);
                }
                if let Some(Some(pos)) = gui_position {
                    let mut positions = HashMap::new();
                    positions.insert(uuid, Point2D::new(pos.0, pos.1));
                    ws_handler.nodes.update_node_positions(positions, graph_id);
                }
                // Fields not mirrored into GraphStore (isometry, alignment, ...) are only shown in the
                // properties panel, which re-fetches on its own via this counter - see its use_resource.
                *NODE_DETAILS_REFRESH.write() += 1;
                if let Some(panel) = panel {
                    panel_requests.push((graph_id, uuid, panel));
                }
                // A patch is a real document mutation, so register its tab as a structural-change
                // candidate (like NodeAdded/NodeRemoved). Undoing a position-only patch from another tab
                // has panel == None and thus no panel_requests entry - without this it would have no jump
                // target and appear to do nothing. Panel-carrying patches still get the more specific
                // panel jump above; the post-loop `if !jumped` guard prevents a double-jump.
                structural_change_graph_ids.push(graph_id);
                primary_structural_graph_id.get_or_insert(graph_id);
            }
            DocumentChange::NodeDetailsChanged {
                uuid,
                graph_id,
                panel,
            } => {
                *NODE_DETAILS_REFRESH.write() += 1;
                panel_requests.push((graph_id, uuid, panel));
            }
            DocumentChange::AnalyzerChanged { .. } => {
                *NODE_DETAILS_REFRESH.write() += 1;
            }
            DocumentChange::AnalyzerMoved { id, gui_position } => {
                // Analyzers live at the root scenery; move the analyzer's canvas node back on
                // undo/redo (a details refresh alone wouldn't touch its position).
                let root_id = *root_graph_id.read();
                let mut positions = HashMap::new();
                positions.insert(id, Point2D::new(gui_position.0, gui_position.1));
                ws_handler.nodes.update_node_positions(positions, root_id);
                // Register the root tab so undoing an analyzer move from another tab switches back to it
                // (same reasoning as the NodePatched arm above).
                structural_change_graph_ids.push(root_id);
                primary_structural_graph_id.get_or_insert(root_id);
            }
            DocumentChange::EdgeAdded {
                graph_id,
                connect_info,
            } => {
                ws_handler.edges.add_edge(connect_info, graph_id);
                structural_change_graph_ids.push(graph_id);
                primary_structural_graph_id.get_or_insert(graph_id);
            }
            DocumentChange::EdgeRemoved {
                graph_id,
                connect_info,
            } => {
                ws_handler.edges.delete_edge(connect_info, graph_id);
                structural_change_graph_ids.push(graph_id);
                primary_structural_graph_id.get_or_insert(graph_id);
            }
            DocumentChange::EdgeUpdated {
                graph_id,
                connect_info,
            } => {
                ws_handler.edges.update_edge(connect_info, graph_id);
                structural_change_graph_ids.push(graph_id);
                primary_structural_graph_id.get_or_insert(graph_id);
            }
            DocumentChange::AnalyzerAdded { analyzer } => {
                let root_id = *root_graph_id.read();
                ws_handler.nodes.add_analyzer_node(
                    NewAnalyzerInfo::from(analyzer.info.clone()),
                    analyzer.id,
                    root_id,
                );
                structural_change_graph_ids.push(root_id);
                primary_structural_graph_id.get_or_insert(root_id);
            }
            DocumentChange::AnalyzerRemoved { id } => {
                let root_id = *root_graph_id.read();
                ws_handler.nodes.remove_nodes(vec![id], root_id);
                structural_change_graph_ids.push(root_id);
                primary_structural_graph_id.get_or_insert(root_id);
            }
            DocumentChange::GraphClosed { graph_id } => {
                // The group was dissolved (e.g. undo of convert-to-group); close its tab if open so
                // the view doesn't keep showing a group that no longer exists. `remove_tabs` also
                // switches away to the root tab if this was the active one.
                ws_handler.workspace.remove_tabs(vec![graph_id]);
            }
            DocumentChange::GraphNeedsRefresh {
                graph_id,
                is_origin,
            } => {
                if workspace.tabs().contains_key(&graph_id) {
                    ws_handler.nodes.clear_graph_store(graph_id);
                    // This re-sync itself never jumps the view by itself, even for an already-open
                    // tab - but its graph_id still feeds into the structural-change tab-jump decision
                    // below, so the overall undo/redo *does* switch there if it isn't already active
                    // (a port-map/structural cascade is a real document mutation, same as an
                    // add/remove - and it can be the *only* signal for a given tab, since
                    // `dedup_against_full_refreshes` drops any `NodeAdded`/`NodeRemoved`/... for a
                    // graph_id a `GraphNeedsRefresh` in the same batch already covers).
                    process_fill_graph_of_group(
                        root_graph_id.into(),
                        graph_id,
                        ws_handler,
                        false,
                        false,
                        workspace,
                    )
                    .await;
                }
                structural_change_graph_ids.push(graph_id);
                // `is_origin` is the backend's own answer to "which of a multi-level port-map
                // cascade's refreshed tabs is the one whose own entry was directly removed" - see
                // `port_map_commands::describe`. Reliable regardless of undo/redo direction, unlike
                // the list order (`Command::Batch::apply` reverses sub-command order between the
                // two - see `undo/mod.rs`).
                if is_origin {
                    primary_structural_graph_id.get_or_insert(graph_id);
                }
            }
            DocumentChange::ViewportChanged {
                graph_id,
                zoom,
                shift,
            } => {
                // Undo/redo of a camera move: switch to that tab (so the user sees it) and restore its
                // pan/zoom. Purely a view change - no document/canvas mutation here.
                if workspace.tabs().contains_key(&graph_id) {
                    ws_handler.workspace.set_active_tab(graph_id);
                    ws_handler.view.set_zoom(graph_id, zoom);
                    ws_handler
                        .view
                        .set_shift(graph_id, Point2D::new(shift.0, shift.1));
                }
            }
        }
    }

    // Picking which of possibly several affected (graph_id, uuid) pairs to select is order-sensitive
    // in a way plain "first reported entry" gets wrong: a `Command::Batch` reverses its sub-commands'
    // order between undo and redo (`Command::Batch::apply`, `undo/mod.rs`), so a multi-tab cascade
    // (e.g. a port-map removal chained through several groups) lists the *same* tabs in opposite order
    // depending on direction - naively always taking the first entry would then pick a *different* tab
    // on redo than undo just jumped to. Preferring `LAST_AUTO_SELECTED_NODE` when it's still one of
    // this response's entries keeps redo consistent with a preceding undo (and vice versa); falling
    // back to the first entry otherwise keeps a *fresh* undo/redo picking the innermost/most-specific
    // one, same as before (`port_map_commands::describe`'s cascade order lists innermost first).
    // Note this is *not* simply "is the active tab already a member of the set": a cascade's outer
    // levels (e.g. root, showing only the group's box) are just as much "members" as the inner one
    // where the real content lives - jumping must still happen even if the active tab happens to be
    // one of the less-specific outer members.
    let chosen_node = (*LAST_AUTO_SELECTED_NODE.read())
        .and_then(|last| {
            panel_requests
                .iter()
                .find(|(g, u, _)| (*g, *u) == last)
                .copied()
        })
        .or_else(|| panel_requests.first().copied());
    let jumped = if let Some((graph_id, uuid, panel)) = chosen_node {
        *LAST_AUTO_SELECTED_NODE.write() = Some((graph_id, uuid));
        let already_showing = *workspace.active_tab().read() == graph_id
            && workspace.tabs().get(graph_id).is_some_and(|g| {
                let selected = g.graph_store().read().get_selected_nodes(graph_id);
                selected.len() == 1 && selected[0].node_id == uuid
            });
        if already_showing {
            false
        } else {
            ensure_tab_active(graph_id, ws_handler, root_graph_id, workspace).await;
            ws_handler.nodes.set_node_active(graph_id, uuid, true, 0);
            *PENDING_PANEL_OPEN.write() = Some((uuid, panel));
            true
        }
    } else {
        false
    };
    // Same idea for the structural (no specific node to select) tab-jump - only if the panel-select
    // above didn't already jump somewhere more specific. Prefer `primary_structural_graph_id`
    // whenever this batch's own data named one directly - reliable regardless of undo/redo
    // direction, since it comes from the response itself rather than being reconstructed
    // client-side. Only fall back to the `LAST_AUTO_JUMPED_GRAPH`-preferred-else-first heuristic
    // when nothing in this batch tags an origin at all (currently `describe_move_nodes`/
    // `describe_group_structure_change`, which don't distinguish one yet).
    if !jumped {
        let chosen_graph = primary_structural_graph_id.or_else(|| {
            (*LAST_AUTO_JUMPED_GRAPH.read())
                .filter(|last| structural_change_graph_ids.contains(last))
                .or_else(|| structural_change_graph_ids.first().copied())
        });
        if let Some(graph_id) = chosen_graph {
            *LAST_AUTO_JUMPED_GRAPH.write() = Some(graph_id);
            if *workspace.active_tab().read() != graph_id {
                ensure_tab_active(graph_id, ws_handler, root_graph_id, workspace).await;
            }
        }
    }
}

/// Makes `graph_id` the active tab if it isn't already, opening it first (via
/// `process_open_group_tab`, fetching its display name) if it isn't open yet - the shared "make sure
/// the user is looking at this tab" primitive, mirroring `process_jump_to_mapped_port`'s tab-switch
/// logic, reused by both the node-editor auto-select feature and structural-change jumps above.
async fn ensure_tab_active(
    graph_id: Uuid,
    ws_handler: WorkSpaceSignalHandlers,
    root_graph_id: Memo<Uuid>,
    workspace: ReadStore<GraphsWorkspaceState>,
) {
    if *workspace.active_tab().read() == graph_id {
        return;
    }
    if workspace.tabs().contains_key(&graph_id) {
        ws_handler.workspace.set_active_tab(graph_id);
    } else if let Ok(group_info) = api::get_node_info(graph_id).await {
        process_open_group_tab(
            graph_id,
            group_info.name,
            ws_handler,
            root_graph_id.into(),
            workspace,
        )
        .await;
    }
}

async fn process_get_editor_area(
    workspace: ReadStore<GraphsWorkspaceState>,
    ws_handler: WorkSpaceSignalHandlers,
) {
    let element_id = format!("editor_{}", workspace.active_tab().read().as_simple());
    let js = format!(
        r"
        let el = document.getElementById('{element_id}');
        if (!el) {{
            dioxus.send(null);
        }} else {{
            let r = el.getBoundingClientRect();
            dioxus.send({{
                x: r.x,
                y: r.y,
                width: r.width,
                height: r.height
            }});
        }}
        "
    );
    let mut eval = dioxus::document::eval(&js);
    if let Ok(rect) = eval.recv::<Value>().await
        && let (Some(x), Some(y), Some(width), Some(height)) = (
            rect["x"].as_f64(),
            rect["y"].as_f64(),
            rect["width"].as_f64(),
            rect["height"].as_f64(),
        )
    {
        let editor_area = Rect::new(Point2D::new(x, y), Size2D::new(width, height));
        ws_handler.workspace.set_editor_area(editor_area);
    }
}

async fn process_jump_to_mapped_port(
    root_scenery_id: ReadSignal<Uuid>,
    workspace: ReadStore<GraphsWorkspaceState>,
    ws_handler: WorkSpaceSignalHandlers,
    mapped_node_id: Uuid,
    parent_id: Uuid,
    parent_name: String,
) {
    let group_tab_already_open = workspace.tabs().contains_key(&parent_id);
    if group_tab_already_open {
        ws_handler.workspace.set_active_tab(parent_id);
    } else {
        process_open_group_tab(
            parent_id,
            parent_name,
            ws_handler,
            root_scenery_id,
            workspace,
        )
        .await;
    }

    ws_handler
        .nodes
        .set_node_active(parent_id, mapped_node_id, true, 0);
}

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

/// Deletes a whole multi-node selection. Optical nodes are removed together via the batch endpoint
/// (`api::delete_nodes`), so one undo restores the whole group; any analyzers in the selection are
/// still deleted individually (they live at document level and are rarely multi-selected with nodes).
async fn process_delete_nodes(
    node_ids: Vec<Uuid>,
    workspace: ReadStore<GraphsWorkspaceState>,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
) {
    let Some(graph) = workspace.tabs().get(graph_id) else {
        OPOSSUM_UI_LOGS.write().add_log(&format!(
            "No graph with id '{}' found",
            graph_id.as_simple()
        ));
        return;
    };
    let (optical, analyzers): (Vec<Uuid>, Vec<Uuid>) = {
        let graph_store = graph.graph_store();
        let store = graph_store.read();
        node_ids
            .into_iter()
            .partition(|id| matches!(store.get_node_type(*id), Some(NodeType::Optical(_))))
    };

    if !optical.is_empty() {
        eval_action_run(
            api::delete_nodes(optical).await,
            Some(move |response: DeleteNodeResponse| {
                ws_handler
                    .nodes
                    .remove_nodes(response.deleted_nodes, graph_id);
                for (group_id, _node_id, external_port_name, port_type) in
                    response.removed_port_mappings
                {
                    ws_handler
                        .workspace
                        .remove_port_map_entry(group_id, external_port_name.clone());
                    ws_handler
                        .nodes
                        .remove_group_port(external_port_name, group_id, port_type);
                }
                for (group_id, edge) in response.disconnected_connections {
                    ws_handler.edges.delete_edge(edge, group_id);
                }
            }),
        );
    }

    for analyzer_id in analyzers {
        eval_action_run(
            api::delete_analyzer(analyzer_id).await,
            Some(move |deleted_id| {
                ws_handler.nodes.remove_nodes(vec![deleted_id], graph_id);
            }),
        );
    }
}

/// Refreshes a single group's external port-map list and its displayed port-name handles from the
/// backend's current state. Used both for a group that was just pasted into (its content is new to the
/// GUI) and for a group that nodes were just cut *out of* (its port maps/handles may have shrunk and need
/// to be reconciled with the now-authoritative backend state).
async fn refresh_group_ports(ws_handler: WorkSpaceSignalHandlers, group_id: Uuid) {
    eval_action_run(
        api::get_port_maps_of_group(group_id).await,
        Some(move |port_mappings_response: PortMappingsResponse| {
            for (group_port_name, (mapped_node_id, mapped_node_port_name)) in
                &port_mappings_response.inputs
            {
                ws_handler.workspace.add_port_map(
                    group_id,
                    group_port_name.clone(),
                    mapped_node_port_name.clone(),
                    *mapped_node_id,
                );
            }
            for (group_port_name, (mapped_node_id, mapped_node_port_name)) in
                &port_mappings_response.outputs
            {
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
        api::get_ports_of_group(group_id).await,
        Some(move |ports_config: NodePortsResponse| {
            let input_ports = ports_config.inputs.into_keys().collect();
            let output_ports = ports_config.outputs.into_keys().collect();
            ws_handler
                .nodes
                .update_group_ports(input_ports, output_ports, group_id);
        }),
    );
}

async fn process_paste_nodes(
    pos: Point2D<f64>,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
    cut_nodes: bool,
    root_scenery_id: Memo<Uuid>,
    workspace: ReadStore<GraphsWorkspaceState>,
) {
    match api::post_paste_nodes(graph_id, pos, cut_nodes).await {
        Ok(PasteNodesResponse {
            pasted_nodes,
            pasted_analyzers,
            pasted_connections,
            cut_result,
        }) => {
            let mut pasted_groups = Vec::<Uuid>::new();
            for (graph_id, n) in &pasted_nodes {
                for node in n {
                    ws_handler.nodes.add_optical_node(node.clone(), *graph_id);
                    if node.node_type() == "group" {
                        pasted_groups.push(node.uuid());
                    }
                }
            }
            for a in &pasted_analyzers {
                let analyzer_id = a.id; // <-- ID aus dem DTO
                ws_handler.nodes.add_analyzer_node(
                    NewAnalyzerInfo::from(a.info.clone()), // <-- info aus dem DTO extrahieren
                    analyzer_id,
                    graph_id,
                );
            }

            let pasted_a_group = !pasted_groups.is_empty();
            for group_id in pasted_groups {
                refresh_group_ports(ws_handler, group_id).await;
            }
            if pasted_a_group {
                // A pasted group's own box, shown in `graph_id` (the tab the paste landed in),
                // needs its ports/mapped marker to appear too. `refresh_group_ports` above patches
                // the box's existing `NodeElement` in place via a cross-tab scan, but that patch
                // isn't triggering a redraw (same known gap already sidestepped for drag-into-group
                // - see `process_drop_nodes_into_group`). Re-fetch `graph_id`'s own children as
                // full `NodeInfo` instead - the same proven-correct mechanism a manual tab
                // close+reopen already goes through. No autolayout/re-centering: this is a
                // background data refresh of a tab the user is already looking at, not a fresh
                // open.
                process_fill_graph_of_group(
                    root_scenery_id.into(),
                    graph_id,
                    ws_handler,
                    false,
                    false,
                    workspace,
                )
                .await;
            }

            for (graph_id, edges) in &pasted_connections {
                for edge in edges {
                    ws_handler.edges.add_edge(edge.clone(), *graph_id);
                }
            }

            if let Some(CutResult {
                deleted_nodes,
                cut_from_group_ids,
                disconnected_connections,
                removed_port_mappings,
            }) = cut_result
            {
                // A multi-select cut can span more than one parent group - apply the removal
                // against each (a no-op for any group a given node doesn't actually belong to).
                for group_id in &cut_from_group_ids {
                    ws_handler
                        .nodes
                        .remove_nodes(deleted_nodes.clone(), *group_id);
                }
                // Only the cut node(s)' own mapping entries are gone - prune exactly those from the
                // GUI's cached port-map list rather than clearing the whole group (which would also drop
                // still-valid mappings of any other, untouched node in the same group). Shrink the
                // group's own port handles by the same precise diff rather than a full refetch - that
                // extra round trip used to be what visually cleared every mapping in the group instead
                // of just the cut node's.
                for (group_id, _node_id, external_port_name, port_type) in removed_port_mappings {
                    ws_handler
                        .workspace
                        .remove_port_map_entry(group_id, external_port_name.clone());
                    ws_handler
                        .nodes
                        .remove_group_port(external_port_name, group_id, port_type);
                }
                // Any external connection that depended on one of those now-gone mappings is also
                // already correctly disconnected server-side - the GUI's own edge list needs the same
                // explicit removal.
                for (group_id, edge) in disconnected_connections {
                    ws_handler.edges.delete_edge(edge, group_id);
                }
            }
        }
        Err(e) => {
            OPOSSUM_UI_LOGS
                .write()
                .add_log(&format!("Error while pasting node/s: {e}"));
        }
    }
}

async fn process_copy_nodes(nodes: HashSet<Uuid>) {
    eval_action_run(api::post_copy_nodes(nodes).await, None::<fn(String)>);
}
async fn process_delete_edge(
    connect_info: ConnectInfo,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
) {
    eval_action_run(
        api::delete_connection(connect_info.clone(), graph_id).await,
        Some(move |()| ws_handler.edges.delete_edge(connect_info, graph_id)),
    );
}
async fn process_update_edge(
    connect_info: ConnectInfo,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
) {
    let update_connection_request = UpdateConnectionRequest {
        src_uuid: connect_info.src_uuid(),
        src_port: connect_info.src_port().to_string(),
        distance: connect_info.distance(),
    };
    eval_action_run(
        api::update_distance(update_connection_request, graph_id).await,
        Some(move |()| ws_handler.edges.update_edge(connect_info, graph_id)),
    );
}
async fn process_optimize_layout(
    workspace: ReadStore<GraphsWorkspaceState>,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
) {
    // --- READ PHASE: Fetch nodes and edges ---
    let (nodes, edges) = {
        let graph = workspace.tabs().get(graph_id);
        let Some(graph) = graph else {
            OPOSSUM_UI_LOGS.write().add_log(&format!(
                "No graph with id '{}' found",
                graph_id.as_simple()
            ));
            return;
        };

        let store = graph.graph_store();
        (store.nodes().read().clone(), store.edges().read().clone())
    };

    // --- CALCULATION PHASE: Determine new positions (Pure) ---
    // Note: ensure optimize_layout is imported from graph_workspace::workspace_state
    let new_positions = optimize_layout(&nodes, &edges);

    // --- ASYNC PHASE: sync with backend, batched into a single undo step ---
    let updates: Vec<PositionUpdate> = new_positions
        .iter()
        .map(|(node_id, pos)| {
            let is_optical = nodes
                .get(node_id)
                .is_none_or(|node| matches!(node.node_type(), NodeType::Optical(_)));
            PositionUpdate {
                uuid: *node_id,
                is_optical,
                gui_position: (pos.x, pos.y),
            }
        })
        .collect();

    // --- WRITE PHASE: update UI state if the sync succeeded ---
    eval_action_run(
        api::patch_positions(updates).await,
        Some(move |()| {
            ws_handler
                .nodes
                .update_node_positions(new_positions, graph_id);
        }),
    );
}

async fn process_add_analyzer(
    analyzer_type: AnalyzerType,
    workspace: ReadStore<GraphsWorkspaceState>,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
) {
    // ----- READ PHASE -----
    let new_analyzer_info = {
        let graph = workspace.tabs().get(graph_id);

        let Some(graph) = graph else {
            OPOSSUM_UI_LOGS.write().add_log(&format!(
                "No graph with id '{}' found",
                graph_id.as_simple()
            ));
            return;
        };

        let editor_state = graph.editor_state();
        let graph_store = graph.graph_store();

        let zoom = *editor_state.zoom().peek();
        let shift = *editor_state.shift().peek();
        let center = workspace.get_view_port_center();

        let proposed_pos = ((center.x - shift.x) / zoom, (center.y - shift.y) / zoom);

        let existing_positions: Vec<_> = graph_store
            .nodes()
            .read()
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

async fn process_add_optic_node(
    new_node_type_string: &str,
    workspace: ReadStore<GraphsWorkspaceState>,
    ws_handler: WorkSpaceSignalHandlers,
    graph_id: Uuid,
) {
    // ----- READ PHASE -----
    let new_node_info = {
        let editor_state_opt = workspace.tabs().get(graph_id).map(|g| g.editor_state());
        let graph_store_opt = workspace.tabs().get(graph_id).map(|g| g.graph_store());

        let (Some(graph_store), Some(editor_state)) = (graph_store_opt, editor_state_opt) else {
            OPOSSUM_UI_LOGS.write().add_log(&format!(
                "No graph with id '{}' found",
                graph_id.as_simple()
            ));
            return;
        };

        let zoom = *editor_state.zoom().peek();

        let shift = *editor_state.shift().peek();
        let center = workspace.get_view_port_center();
        let proposed_pos = (
            (center.x - shift.x - NODE_WIDTH / 2.) / zoom,
            (center.y - shift.y - f64::midpoint(MIN_NODE_BODY_HEIGHT, HEADER_HEIGHT)) / zoom,
        );

        let existing_positions: Vec<_> = graph_store
            .nodes()
            .read()
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
        Some(move |node_info: NodeInfo| {
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

async fn process_remove_port_map(
    group_id: Uuid,
    group_port_name: String,
    port_type: PortType,
    ws_handler: WorkSpaceSignalHandlers,
) {
    match api::remove_port_map(group_port_name, group_id, port_type).await {
        Ok(response) => {
            if response.port_removed {
                // Removing a mapping can cascade outward through however many groups it's
                // chained through (see `remove_port_map_cascade` on the backend) - apply exactly
                // what's reported for each level: prune the internal "mapped" bookkeeping and
                // shrink that group's own displayed port handle, same pattern already used for a
                // deleted node's port mapping (`process_delete_node`).
                for (level_group_id, _node_id, external_port_name, level_port_type) in
                    response.removed_port_mappings
                {
                    ws_handler
                        .workspace
                        .remove_port_map_entry(level_group_id, external_port_name.clone());
                    ws_handler.nodes.remove_group_port(
                        external_port_name,
                        level_group_id,
                        level_port_type,
                    );
                }
                for (owning_group_id, edge) in response.disconnected_connections {
                    ws_handler.edges.delete_edge(edge, owning_group_id);
                }
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
        Ok(response) => {
            ws_handler.workspace.add_port_map(
                group_id,
                group_port_name,
                mapped_node_port_name,
                mapped_node_id,
            );
            ws_handler
                .nodes
                .update_group_ports(response.inputs, response.outputs, group_id);
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }
}

/// Silently seed a tab for `group_id` if it doesn't exist yet (e.g. a subgroup that was just
/// created but never opened), so subsequent writes into its own graph store - nodes, edges, port
/// maps - actually land instead of silently no-op'ing against a tab that was never created. Does
/// not touch `tab_order`/`active_tab`, so it never pops open a tab the user didn't ask for.
async fn ensure_group_tab_exists(
    group_id: Uuid,
    ws_handler: WorkSpaceSignalHandlers,
    workspace: ReadStore<GraphsWorkspaceState>,
) {
    if workspace.tabs().contains_key(&group_id) {
        return;
    }
    if let Ok(hierarchy) = api::get_group_hierarchy(group_id).await {
        let name = hierarchy
            .last()
            .map(|(_, name)| name.clone())
            .unwrap_or_default();
        ws_handler.workspace.ensure_group_tab(GraphInfo {
            name,
            id: group_id,
            hierarchy,
        });
    }
}

async fn process_drop_nodes_into_group(
    nodes: Vec<Uuid>,
    from_group_id: Uuid,
    drop_group_id: Uuid,
    ws_handler: WorkSpaceSignalHandlers,
    root_scenery_id: Memo<Uuid>,
    workspace: ReadStore<GraphsWorkspaceState>,
) {
    match api::drop_nodes_into_group(nodes.clone(), from_group_id, drop_group_id).await {
        Ok(response) => {
            ws_handler.nodes.remove_nodes(nodes, from_group_id);
            // A moved node's connection to a sibling left behind, or to an external node via a
            // pre-existing port mapping, is preserved rather than dropped - rerouted through a new
            // mapping on the destination group (or reconnected directly if the other endpoint already
            // lives there). Reflect exactly what the backend reports: new edges, torn-down old ones, and
            // any port-map entry removed with no replacement under the same name (a purely additive
            // refresh below wouldn't otherwise notice a key that's simply gone).
            for (group_id, edge) in response.new_connections {
                ws_handler.edges.add_edge(edge, group_id);
            }
            for (group_id, edge) in response.removed_connections {
                ws_handler.edges.delete_edge(edge, group_id);
            }
            for (group_id, _node_id, external_port_name, _port_type) in
                response.removed_port_mappings
            {
                ws_handler
                    .workspace
                    .remove_port_map_entry(group_id, external_port_name);
            }
            // The destination group (and any other group whose port map changed) may never have
            // been opened before - make sure its tab exists before writing into it below.
            for group_id in response
                .port_map_groups_changed
                .iter()
                .copied()
                .chain(std::iter::once(drop_group_id))
            {
                ensure_group_tab_exists(group_id, ws_handler, workspace).await;
            }
            for group_id in response.port_map_groups_changed {
                refresh_group_ports(ws_handler, group_id).await;
            }

            process_fill_graph_of_group(
                root_scenery_id.into(),
                drop_group_id,
                ws_handler,
                false,
                true,
                workspace,
            )
            .await;

            // The subgroup's box, as shown in `from_group_id`'s own (already-open) tab, needs its
            // new port(s) and "mapped" marker to appear too. `refresh_group_ports` above patches
            // the box's existing `NodeElement` in place via a cross-tab scan, but that patch isn't
            // triggering a redraw. Re-fetch `from_group_id`'s own children as full `NodeInfo`
            // instead - the same proven-correct mechanism a manual tab close+reopen already goes
            // through - to rebuild the subgroup's `NodeElement` (ports included) from scratch. No
            // autolayout/re-centering: this is a background data refresh of a tab the user is
            // already actively looking at, not a fresh open.
            process_fill_graph_of_group(
                root_scenery_id.into(),
                from_group_id,
                ws_handler,
                false,
                false,
                workspace,
            )
            .await;

            // If the moved node had its own external mapping, `from_group_id`'s own port map got
            // repointed too (same external name, new internal target) - so `from_group_id`'s own
            // box, as shown one level further out in *its* parent's tab, needs the same redraw
            // sidestep as above. `GraphInfo::get_parent` has its own off-by-one for a group that's
            // a *direct* child of the root (returns `None` instead of `Some(root)`) - mirror the
            // same root-id fallback `hooks.rs`'s context menu already uses for that case, and skip
            // entirely when `from_group_id` is the root itself (calling `get_parent` on a
            // single-entry hierarchy underflows).
            let root_id = *root_scenery_id.read();
            if from_group_id != root_id {
                let from_group_parent_id = workspace
                    .tabs()
                    .get(from_group_id)
                    .and_then(|g| g.graph_info().read().get_parent())
                    .map_or(root_id, |(id, _)| id);
                process_fill_graph_of_group(
                    root_scenery_id.into(),
                    from_group_parent_id,
                    ws_handler,
                    false,
                    false,
                    workspace,
                )
                .await;
            }
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }

    ws_handler.nodes.remove_droppable_group();
}

async fn process_convert_nodes_to_group(
    nodes: Vec<Uuid>,
    current_group_id: Uuid,
    ws_handler: WorkSpaceSignalHandlers,
    workspace: ReadStore<GraphsWorkspaceState>,
) {
    if !nodes.is_empty() {
        // guard, if the nodes vector is empty (e.g. all nodes filtered out before)
        match api::convert_nodes_to_group(nodes.clone(), current_group_id).await {
            Ok(response) => {
                let new_group_id = response.new_group.uuid();

                // remove nodes that have been converted to a group from graph
                ws_handler.nodes.remove_nodes(nodes, current_group_id);

                // Add the new group node - built server-side from a `NodeInfo` already fully
                // resolved (ports included, reflecting any pre-existing mapping a converted node
                // had that got rerouted through it), so its box shows correctly immediately.
                // Unlike patching an *existing* node's ports in place cross-tab (the known
                // `update_group_ports_handler` redraw gap - see `process_drop_nodes_into_group`),
                // inserting a brand-new node doesn't hit that issue: the node list's key set
                // changes, which does reliably trigger a redraw.
                ws_handler
                    .nodes
                    .add_optical_node(response.new_group, current_group_id);

                // Reflect exactly what the backend reports: a boundary sibling reconnected
                // through the new group, any edge torn down as a side effect, and any port-map
                // entry removed with no replacement under the same name.
                for (group_id, edge) in response.new_connections {
                    ws_handler.edges.add_edge(edge, group_id);
                }
                for (group_id, edge) in response.removed_connections {
                    ws_handler.edges.delete_edge(edge, group_id);
                }
                for (group_id, _node_id, external_port_name, _port_type) in
                    response.removed_port_mappings
                {
                    ws_handler
                        .workspace
                        .remove_port_map_entry(group_id, external_port_name);
                }

                // The new group's own tab may never be opened - seed it (same as
                // `process_drop_nodes_into_group` does for its drop target) so its internal
                // port-map bookkeeping is correct regardless, and so `current_group_id`'s own
                // port-map cache picks up the new group as the rerouted target for a converted
                // node's pre-existing mapping, if any.
                for group_id in response
                    .port_map_groups_changed
                    .iter()
                    .copied()
                    .chain(std::iter::once(new_group_id))
                {
                    ensure_group_tab_exists(group_id, ws_handler, workspace).await;
                }
                for group_id in response.port_map_groups_changed {
                    refresh_group_ports(ws_handler, group_id).await;
                }
            }
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
            }
        }
    }
}

async fn process_open_group_tab(
    group_id: Uuid,
    group_name: String,
    ws_handler: WorkSpaceSignalHandlers,
    root_scenery_id: ReadSignal<Uuid>,
    workspace: ReadStore<GraphsWorkspaceState>,
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

    process_fill_graph_of_group(
        root_scenery_id,
        group_id,
        ws_handler,
        false,
        true,
        workspace,
    )
    .await;
}

async fn process_fill_graph_of_group(
    root_scenery_id: ReadSignal<Uuid>,
    group_id: Uuid,
    ws_handler: WorkSpaceSignalHandlers,
    needs_autolayout: bool,
    should_center: bool,
    workspace: ReadStore<GraphsWorkspaceState>, // <-- Neu: Workspace
) {
    eval_action_run(
        api::get_nodes(group_id).await,
        Some(move |nodes: Vec<NodeInfo>| ws_handler.nodes.add_group_nodes(group_id, nodes)),
    );

    eval_action_run(
        api::get_port_maps_of_group(group_id).await,
        Some(move |port_mappings_response: PortMappingsResponse| {
            // ... (Dein existierender Code für Port-Mappings)
            for (group_port_name, (mapped_node_id, mapped_node_port_name)) in
                &port_mappings_response.inputs
            {
                ws_handler.workspace.add_port_map(
                    group_id,
                    group_port_name.clone(),
                    mapped_node_port_name.clone(),
                    *mapped_node_id,
                );
            }
            for (group_port_name, (mapped_node_id, mapped_node_port_name)) in
                &port_mappings_response.outputs
            {
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

            // Layout für Sub-Gruppen starten
            if needs_autolayout && *root_scenery_id.read() != group_id {
                dioxus::prelude::spawn(async move {
                    process_optimize_layout(workspace, ws_handler, group_id).await;
                });
            }
        }),
    );

    if *root_scenery_id.read() == group_id {
        eval_action_run(
            api::get_analyzers().await,
            Some(move |analyzers: Vec<AnalyzerItemDto>| {
                ws_handler.nodes.add_group_analyzers(group_id, analyzers);

                // Layout für die Root-Scenery starten
                if needs_autolayout {
                    dioxus::prelude::spawn(async move {
                        process_optimize_layout(workspace, ws_handler, group_id).await;
                    });
                }
            }),
        );
    }

    if should_center {
        ws_handler.view.center_graph(group_id, false);
    }
}

async fn process_load_from_file(
    workspace: ReadStore<GraphsWorkspaceState>,
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

    match api::put_document(opm_string).await {
        Ok(response) => {
            process_add_root_scenery_tab(workspace, ws_handler, response.name).await;
            set_file_path_handler.call(Some(path));

            let scenery_id = *scenery_id_sig.read();

            process_fill_graph_of_group(
                scenery_id_sig.into(),
                scenery_id,
                ws_handler,
                response.needs_autolayout,
                true,
                workspace, // <-- Workspace durchreichen
            )
            .await;
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }
}

async fn process_delete_root_scenery(
    workspace_handlers: WorkSpaceSignalHandlers,
    set_file_path_handler: EventHandler<Option<PathBuf>>,
) {
    eval_action_run(
        delete_document().await,
        Some(move |_| {
            workspace_handlers.workspace.clear_workspace();
            set_file_path_handler.call(None);
        }),
    );
}

async fn process_save_root_scenery_to_file(
    path: PathBuf,
    set_file_path_handler: EventHandler<Option<PathBuf>>,
    ws_handler: WorkSpaceSignalHandlers,
    root_id: Uuid,
    workspace: ReadStore<GraphsWorkspaceState>,
) {
    if let Some(f_stem) = path.file_stem()
        && let Some(fname) = f_stem.to_str()
    {
        // Only rename the root scenery when its name actually differs from the file's - the rename
        // is pure bookkeeping (`patch_node` deliberately records no undo step for root patches),
        // so skipping the no-op case just avoids a pointless request.
        let current_root_name = workspace
            .tabs()
            .get(root_id)
            .map(|g| g.graph_info().read().name.clone());
        if current_root_name.as_deref() != Some(fname) {
            process_rename_root_scenery(ws_handler, fname.to_string(), root_id, false).await;
        }
        eval_action_run(
            api::get_document().await,
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

async fn process_add_root_scenery_tab(
    workspace: ReadStore<GraphsWorkspaceState>,
    ws_handler: WorkSpaceSignalHandlers,
    name: String,
) {
    match api::get_document_root_uuid().await {
        Ok(id) => {
            ws_handler.workspace.clear_workspace();
            ws_handler.workspace.set_root_scenery_id(id);
            ws_handler.workspace.add_new_group_tab(GraphInfo {
                name: name.clone(),
                id,
                hierarchy: vec![(id, name.clone())],
            });
            process_rename_root_scenery(ws_handler, name, id, false).await;
            process_get_editor_area(workspace, ws_handler).await;
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }
}

async fn process_rename_root_scenery(
    ws_handler: WorkSpaceSignalHandlers,
    name: String,
    root_id: Uuid,
    needs_saving: bool,
) {
    eval_action_run(
        api::update_node_name(root_id, &name).await,
        Some(move |()| {
            ws_handler
                .nodes
                .set_node_name(name.clone(), root_id, root_id, needs_saving);
        }),
    );
}
