use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use web_time::Instant;

use crate::{
    CONTEXT_MENU, api,
    components::scenery_editor::{
        GraphState, NodeType,
        constants::{MAX_ZOOM, MIN_ZOOM, ZOOM_SENSITIVITY},
        graph_workspace::{
            DragStatus, EditorStateStoreExt, GraphStateStoreExt, GraphStoreStoreExt,
            GraphsWorkspaceAction, GraphsWorkspaceState, GraphsWorkspaceStateStoreExt,
        },
    },
};
use dioxus::{
    html::{geometry::euclid::default::Point2D, input_data::MouseButton},
    prelude::*,
};
use opossum_core::{
    prelude::*,
    types::api_types::{ConnectInfo, Viewport},
};
use uuid::Uuid;

pub fn use_zoom() -> impl FnMut(WheelEvent) {
    let graph_state = use_context::<ReadStore<GraphState>>();
    let editor_status = graph_state.editor_state();
    let workspace = use_context::<ReadStore<GraphsWorkspaceState>>();
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    // Debounce state for the undo-recording POST (see the send site below): the viewport at the start of
    // the current scroll burst, the latest viewport, and a generation counter so only the newest wheel
    // tick's task actually sends.
    let mut gesture_start = use_signal(|| None::<Viewport>);
    let mut latest = use_signal(|| None::<Viewport>);
    let mut debounce_gen = use_signal(|| 0u64);

    move |wheel_event| {
        let current_graph_zoom = *editor_status.zoom().read();
        let current_graph_shift = *editor_status.shift().read();
        let rect = *workspace.editor_area().read();
        let client_pos = wheel_event.data.client_coordinates();
        let mouse_pos = Point2D::new(client_pos.x - rect.min_x(), client_pos.y - rect.min_y());
        let mouse_on_graph_x = (mouse_pos.x - current_graph_shift.x) / current_graph_zoom;
        let mouse_on_graph_y = (mouse_pos.y - current_graph_shift.y) / current_graph_zoom;
        let delta = wheel_event.delta().strip_units().y;
        let new_graph_zoom = if delta > 0.0 {
            (current_graph_zoom * ZOOM_SENSITIVITY).min(MAX_ZOOM)
        } else {
            (current_graph_zoom / ZOOM_SENSITIVITY).max(MIN_ZOOM)
        };
        let new_shift_x = mouse_on_graph_x.mul_add(-new_graph_zoom, mouse_pos.x);
        let new_shift_y = mouse_on_graph_y.mul_add(-new_graph_zoom, mouse_pos.y);

        let graph_id = *workspace.active_tab().read();

        let before = Viewport {
            graph_id,
            zoom: current_graph_zoom,
            shift: (current_graph_shift.x, current_graph_shift.y),
        };
        let after = Viewport {
            graph_id,
            zoom: new_graph_zoom,
            shift: (new_shift_x, new_shift_y),
        };
        // At the MIN/MAX zoom clamp the tick doesn't move the camera (before == after). The backend would
        // discard it anyway, so skip the no-op SetZoom/SetShift and - crucially - the optimistic
        // Undo-enable below, which would otherwise light up the Undo button for a no-op (and clicking it
        // 409s on an empty stack). Mirrors `push_viewport_change`'s own before==after guard.
        if before == after {
            return;
        }

        workspace_processor.send(GraphsWorkspaceAction::SetZoom {
            graph_id,
            zoom: new_graph_zoom,
        });
        workspace_processor.send(GraphsWorkspaceAction::SetShift {
            graph_id,
            shift: Point2D::new(new_shift_x, new_shift_y),
        });

        // A zoom is undoable, so enable Undo / grey out Redo like any other edit.
        *crate::UNDO_REDO_STATUS.write() = (true, false);

        // Record this as an undo step, but debounce the backend POST: a scroll burst is dozens of ticks, so
        // instead of posting each one we cache the burst's start/end and send a single request once ~120ms
        // pass with no further tick. The local SetZoom/SetShift above are applied every tick, so the view
        // stays smooth in the meantime.
        if gesture_start.peek().is_none() {
            gesture_start.set(Some(before));
        }
        latest.set(Some(after));
        let generation = *debounce_gen.peek() + 1;
        debounce_gen.set(generation);
        spawn(async move {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Native platform (Desktop): use tokio
                tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            }
            #[cfg(target_arch = "wasm32")]
            {
                // Web platform (WASM): use gloo_timers
                gloo_timers::future::sleep(std::time::Duration::from_millis(120)).await;
            }

            // Only the newest tick's task flushes; a later tick bumps the generation, superseding earlier
            // tasks, so those just return.
            if *debounce_gen.peek() != generation {
                return;
            }
            let start = gesture_start.write().take();
            let end = latest.write().take();
            if let (Some(start), Some(end)) = (start, end) {
                // coalesce=true so consecutive scroll bursts still combine into one undo step (see
                // `post_viewport_change`); the burst itself is now a single request.
                let _ = api::post_viewport_change(start, end, true, false).await;
            }
        });
    }
}

pub fn use_on_mouse_down(
    mut current_mouse_pos: Signal<Point2D<f64>>,
    mut last_click: Signal<Option<Instant>>,
    ctrl_pressed: ReadSignal<bool>,
    graph_id: Uuid,
) -> impl FnMut(MouseEvent) {
    let dc_time = Duration::from_millis(300);
    let graph_state = use_context::<ReadStore<GraphState>>();
    let editor_status = graph_state.editor_state();
    let graph_store = graph_state.graph_store();
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let workspace = use_context::<ReadStore<GraphsWorkspaceState>>();

    move |event: MouseEvent| {
        event.stop_propagation();
        if let Some(trigger_button) = event.trigger_button() {
            match trigger_button {
                MouseButton::Primary => {
                    let mut ctx = CONTEXT_MENU.write();
                    *ctx = None;

                    let selected = graph_store.node_selection().peek().clone();
                    if !ctrl_pressed() && !selected.all_nodes.is_empty() {
                        workspace_processor
                            .send(GraphsWorkspaceAction::ClearSelectedNodes { graph_id });
                    }
                    let mouse_pos =
                        Point2D::new(event.client_coordinates().x, event.client_coordinates().y);

                    let editor_origin = workspace.editor_area().read().origin;
                    let current_shift = *editor_status.shift().read();
                    let current_zoom = *editor_status.zoom().read();

                    let rect_origin = Point2D::new(
                        (mouse_pos.x - editor_origin.x - current_shift.x) / current_zoom,
                        (mouse_pos.y - editor_origin.y - current_shift.y) / current_zoom,
                    );

                    let drag_status = DragStatus::ArmedSelection(rect_origin);
                    workspace_processor.send(GraphsWorkspaceAction::SetDragStatus(drag_status));
                }
                MouseButton::Auxiliary => {
                    //for dragging
                    current_mouse_pos.set(Point2D::new(
                        event.client_coordinates().x,
                        event.client_coordinates().y,
                    ));
                    workspace_processor
                        .send(GraphsWorkspaceAction::SetDragStatus(DragStatus::Graph));

                    // for double-click zoom
                    event.stop_propagation();
                    let now = Instant::now();
                    let t0_opt = *last_click.read();
                    if let Some(t0) = t0_opt
                        && now.duration_since(t0) < dc_time
                    {
                        let graph_id = *workspace.active_tab().read();
                        workspace_processor.send(GraphsWorkspaceAction::CenterGraph {
                            graph_id,
                            save_changes: true,
                            record_undo: true,
                        });
                        last_click.set(None);
                    }
                    last_click.set(Some(now));
                }
                _ => (),
            }
        }
    }
}
pub fn use_drag(mut current_mouse_pos: Signal<Point2D<f64>>) -> impl FnMut(MouseEvent) {
    let graph_state = use_context::<ReadStore<GraphState>>();
    let editor_status = graph_state.editor_state();
    let workspace = use_context::<ReadStore<GraphsWorkspaceState>>();
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let graph_id = graph_state.graph_info().read().id;
    let drag_status = workspace.drag_status();

    move |event| {
        let current_shift = *editor_status.shift().read();
        let relative_shift = Point2D::new(
            event.client_coordinates().x - current_mouse_pos().x,
            event.client_coordinates().y - current_mouse_pos().y,
        );
        let mouse_pos = Point2D::new(event.client_coordinates().x, event.client_coordinates().y);
        current_mouse_pos.set(mouse_pos);

        if *drag_status.read() == DragStatus::NodeInit || *drag_status.read() == DragStatus::None {
            return;
        }
        let mouse_to_graph_shift =
            Point2D::new(mouse_pos.x - current_shift.x, mouse_pos.y - current_shift.y);

        workspace_processor.send(GraphsWorkspaceAction::ApplyDrag {
            graph_id,
            relative_shift,
            current_zoom: *editor_status.zoom().read(),
            mouse_to_graph_shift,
        });
    }
}

pub fn use_on_key_up(
    mut ctrl_pressed: Signal<bool>,
    mut shift_pressed: Signal<bool>,
) -> impl FnMut(KeyboardEvent) {
    move |event| {
        if event.key() == Key::Control {
            ctrl_pressed.set(false);
        }
        if event.key() == Key::Shift {
            shift_pressed.set(false);
        }
    }
}

pub fn use_on_key_down(
    mouse_pos: Signal<Point2D<f64>>,
    workspace: ReadStore<GraphsWorkspaceState>,
    mut ctrl_pressed: Signal<bool>,
    mut shift_pressed: Signal<bool>,
) -> impl FnMut(KeyboardEvent) {
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    move |event| {
        let active_graph = *workspace.active_tab().read();
        if let Some(graph_state) = workspace.tabs().get(active_graph) {
            let editor_status = graph_state.editor_state();
            let graph_store = graph_state.graph_store();
            if !event.is_auto_repeating() {
                let modifiers = event.modifiers();
                let ctrl_or_meta = modifiers.ctrl() || modifiers.meta();

                if modifiers.ctrl() {
                    ctrl_pressed.set(true);
                }
                if modifiers.shift() {
                    shift_pressed.set(true);
                }

                if ctrl_or_meta
                    && !modifiers.shift()
                    && event.data().key() == Key::Character("c".to_string())
                    && !graph_store.read().selected_nodes().is_empty()
                {
                    workspace_processor.send(GraphsWorkspaceAction::CopyNodes {
                        nodes: graph_store.read().selected_node_ids(),
                    });
                    event.stop_propagation();
                } else if ctrl_or_meta
                    && !modifiers.shift()
                    && event.data().key() == Key::Character("x".to_string())
                {
                    workspace_processor.send(GraphsWorkspaceAction::CutNodes {
                        nodes: graph_store.read().selected_node_ids(),
                    });
                    event.stop_propagation();
                } else if ctrl_or_meta
                    && !modifiers.shift()
                    && event.data().key() == Key::Character("v".to_string())
                {
                    let rect = *workspace.editor_area().read();
                    let mouse = *mouse_pos.read();
                    if mouse.x > rect.min_x()
                        && mouse.x < rect.max_x()
                        && mouse.y > rect.min_y()
                        && mouse.y < rect.max_y()
                    {
                        let shift = *editor_status.shift().read();
                        let zoom = *editor_status.zoom().read();
                        let pos = Point2D::new(
                            (mouse.x - shift.x - rect.min_x()) / zoom,
                            (mouse.y - shift.y - rect.min_y()) / zoom,
                        );
                        workspace_processor.send(GraphsWorkspaceAction::PasteNode {
                            pos,
                            graph_id: graph_state.graph_info().read().id,
                        });
                    }
                    event.stop_propagation();
                } else if event.data().key() == Key::Delete {
                    let node_ids: Vec<Uuid> = graph_store
                        .read()
                        .selected_nodes()
                        .keys()
                        .copied()
                        .collect();
                    if !node_ids.is_empty() {
                        // One action for the whole selection, so deleting several nodes at once is a
                        // single undo step instead of one per node.
                        workspace_processor.send(GraphsWorkspaceAction::DeleteNodes {
                            node_ids,
                            graph_id: graph_state.graph_info().read().id,
                        });
                    }

                    event.stop_propagation();
                } else if ctrl_or_meta
                    && !modifiers.shift()
                    && event.data().key() == Key::Character("z".to_string())
                {
                    // Undo is handled by the global JS shortcut listener; clear ctrl_pressed
                    // here so it does not get stuck if focus cycles during the async re-render.
                    ctrl_pressed.set(false);
                } else if ctrl_or_meta
                    && !modifiers.shift()
                    && event.data().key() == Key::Character("y".to_string())
                {
                    ctrl_pressed.set(false);
                }
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
pub fn use_drag_end(
    workspace: ReadStore<GraphsWorkspaceState>,
    nodes_in_selection: Option<HashSet<Uuid>>,
) -> impl FnMut(MouseEvent) {
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    move |_| {
        let active_graph = *workspace.active_tab().read();
        if let Some(graph_state) = workspace.tabs().get(active_graph) {
            let editor_status = graph_state.editor_state();
            let graph_store = graph_state.graph_store();
            let drag_status = workspace.drag_status().read().clone();
            let droppable_groups = *workspace.drop_in_group().read();

            if drag_status != DragStatus::None {
                workspace_processor.send(GraphsWorkspaceAction::SetDragStatus(DragStatus::None));
            }

            match drag_status {
                DragStatus::Nodes => {
                    if droppable_groups.is_none() {
                        let selected_nodes = graph_store().selected_nodes();
                        let nodes_field = graph_store.nodes();
                        let nodes = nodes_field.read();
                        let moves: Vec<(Uuid, bool, Point2D<f64>)> = selected_nodes
                            .keys()
                            .filter_map(|node_id| {
                                nodes
                                    .get(node_id)
                                    .map(|n| (*node_id, n.is_optical_node(), n.pos()))
                            })
                            .collect();
                        drop(nodes);
                        if !moves.is_empty() {
                            workspace_processor
                                .send(GraphsWorkspaceAction::SyncNodePositions { moves });
                        }
                    } else if let Some((to_graph_id, _)) = droppable_groups {
                        let selected_optical_nodes = graph_store().selected_optical_nodes();
                        workspace_processor.send(GraphsWorkspaceAction::DropNodesIntoGroup {
                            nodes: selected_optical_nodes.iter().copied().collect(),
                            from_graph_id: active_graph,
                            to_graph_id,
                        });
                    }
                }
                DragStatus::SelectionBox(_) => {
                    if let Some(nodes_in_selection) = &nodes_in_selection {
                        let current_selection =
                            graph_store.node_selection().read().all_nodes.read().clone();

                        let nodes_to_remove: HashSet<Uuid> = nodes_in_selection
                            .iter()
                            .filter(|id| current_selection.contains_key(id))
                            .copied()
                            .collect();

                        let nodes = graph_store.nodes().read().clone();
                        let nodes_to_add: HashMap<Uuid, bool> = nodes_in_selection
                            .iter()
                            .filter(|id| !current_selection.contains_key(id))
                            .filter_map(|id| {
                                let node = nodes.get(id)?;

                                let is_optical = matches!(node.node_type(), NodeType::Optical(_));
                                Some((*id, is_optical))
                            })
                            .collect();

                        for (node_id, is_optical) in nodes_to_add {
                            workspace_processor.send(GraphsWorkspaceAction::AddToNodeSelection {
                                graph_id: active_graph,
                                node_id,
                                is_optical,
                            });
                        }

                        for node_id in nodes_to_remove {
                            workspace_processor.send(
                                GraphsWorkspaceAction::RemoveFromNodeSelection {
                                    graph_id: active_graph,
                                    node_id,
                                },
                            );
                        }
                    }

                    workspace_processor.send(GraphsWorkspaceAction::SetSelectionBox(None));
                }
                DragStatus::Edge(_) => {
                    if let Some(edge) = editor_status.edge_in_creation().read().clone()
                        && edge.is_valid()
                        && let (Some(end_port), start_port) = (edge.end_port(), edge.start_port())
                    {
                        let (start_port, end_port) = if start_port.port_type == PortType::Output {
                            (start_port, end_port)
                        } else {
                            (end_port, start_port)
                        };

                        let new_edge = ConnectInfo::new(
                            start_port.node_id,
                            start_port.port_name.clone(),
                            end_port.node_id,
                            end_port.port_name.clone(),
                            0.0,
                            false,
                        );
                        workspace_processor.send(GraphsWorkspaceAction::AddEdge {
                            new_edge,
                            graph_id: graph_state.graph_info().read().id,
                        });
                    }
                    workspace_processor.send(GraphsWorkspaceAction::SetEdgeInCreation {
                        graph_id: graph_state.graph_info().read().id,
                        edge_in_creation: None,
                    });
                }
                _ => {}
            }
        }
    }
}
