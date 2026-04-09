use std::time::{Duration, Instant};

use crate::{
    CONTEXT_MENU,
    components::scenery_editor::{
        GraphState, NodeElement,
        constants::{MAX_ZOOM, MIN_ZOOM, ZOOM_SENSITIVITY},
        graph_workspace::{DragStatus, EditorState, GraphsWorkspaceAction, GraphsWorkspaceState},
    },
};
use dioxus::{
    html::{
        geometry::euclid::default::{Point2D, Rect, Size2D},
        input_data::MouseButton,
    },
    prelude::*,
};
use opossum_core::{prelude::*, types::api_types::ConnectInfo};
use uuid::Uuid;

pub fn use_zoom() -> impl FnMut(WheelEvent) {
    let editor_status = use_context::<ReadSignal<EditorState>>();
    let workspace = use_context::<ReadSignal<GraphsWorkspaceState>>();
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();

    move |wheel_event| {
        let zoom = editor_status().zoom;
        let shift = editor_status().shift;
        let rect = *workspace.read().editor_area.read();
        let client_pos = wheel_event.data.client_coordinates();
        let mouse_pos = Point2D::new(client_pos.x - rect.min_x(), client_pos.y - rect.min_y());
        let current_graph_shift = *shift.read();
        let current_graph_zoom = *zoom.read();
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

        let graph_id = *workspace.read().active_tab.read();
        workspace_processor.send(GraphsWorkspaceAction::SetZoom {
            graph_id,
            zoom: new_graph_zoom,
        });
        workspace_processor.send(GraphsWorkspaceAction::SetShift {
            graph_id,
            shift: Point2D::new(new_shift_x, new_shift_y),
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
    let editor_status = use_context::<ReadSignal<EditorState>>();
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let workspace = use_context::<ReadSignal<GraphsWorkspaceState>>();

    move |event: MouseEvent| {
        event.stop_propagation();
        if let Some(trigger_button) = event.trigger_button() {
            match trigger_button {
                MouseButton::Primary => {
                    let mut ctx = CONTEXT_MENU.write();
                    *ctx = None;

                    if ctrl_pressed() {
                        workspace_processor
                            .send(GraphsWorkspaceAction::ClearNodesToBeRemoved { graph_id });
                    } else {
                        workspace_processor
                            .send(GraphsWorkspaceAction::ClearNodesToBeSelected { graph_id });
                        workspace_processor
                            .send(GraphsWorkspaceAction::ClearSelectedNodes { graph_id });
                    }
                    let mouse_pos =
                        Point2D::new(event.client_coordinates().x, event.client_coordinates().y);

                    let editor_origin = workspace().editor_area.read().origin;
                    let current_shift = *editor_status().shift.read();
                    let current_zoom = *editor_status().zoom.read();

                    let rect_origin = Point2D::new(
                        (mouse_pos.x - editor_origin.x - current_shift.x) / current_zoom,
                        (mouse_pos.y - editor_origin.y - current_shift.y) / current_zoom,
                    );

                    let drag_status =
                        DragStatus::SelectionBox(Rect::new(rect_origin, Size2D::new(0., 0.)));
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
                        let graph_id = *workspace.read().active_tab.read();
                        workspace_processor.send(GraphsWorkspaceAction::CenterGraph {
                            graph_id,
                            save_changes: true,
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
    let editor_status = use_context::<ReadSignal<EditorState>>();
    let workspace = use_context::<ReadSignal<GraphsWorkspaceState>>();
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let graph_id = use_context::<ReadSignal<GraphState>>().read().graph_info.id;

    move |event| {
        let current_shift = *editor_status().shift.read();
        let relative_shift = Point2D::new(
            event.client_coordinates().x - current_mouse_pos().x,
            event.client_coordinates().y - current_mouse_pos().y,
        );

        let mouse_pos = Point2D::new(event.client_coordinates().x, event.client_coordinates().y);
        current_mouse_pos.set(mouse_pos);

        let mouse_to_graph_shift =
            Point2D::new(mouse_pos.x - current_shift.x, mouse_pos.y - current_shift.y);

        workspace_processor.send(GraphsWorkspaceAction::ApplyDrag {
            graph_id,
            drag_status: workspace.read().drag_status.read().clone(),
            relative_shift,
            current_zoom: *editor_status().zoom.read(),
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
    workspace: ReadSignal<GraphsWorkspaceState>,
    mut ctrl_pressed: Signal<bool>,
    mut shift_pressed: Signal<bool>,
) -> impl FnMut(KeyboardEvent) {
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    move |event| {
        let active_graph = workspace.read().active_tab;
        if let Some(graph_state) = workspace.read().tabs.read().get(&*active_graph.read()) {
            let editor_status = graph_state.read().editor_state;
            let graph_store = graph_state.read().graph_store;
            if !event.is_auto_repeating() {
                let modifiers = event.modifiers();
                let ctrl_or_meta = modifiers.ctrl() || modifiers.meta();

                if modifiers.ctrl() {
                    ctrl_pressed.set(true);
                }
                if modifiers.ctrl() {
                    shift_pressed.set(false);
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
                    let rect = *workspace().editor_area.read();
                    let mouse = *mouse_pos.read();
                    if mouse.x > rect.min_x()
                        && mouse.x < rect.max_x()
                        && mouse.y > rect.min_y()
                        && mouse.y < rect.max_y()
                    {
                        let shift = *editor_status().shift.read();
                        let zoom = *editor_status().zoom.read();
                        let pos = Point2D::new(
                            (mouse.x - shift.x - rect.min_x()) / zoom,
                            (mouse.y - shift.y - rect.min_y()) / zoom,
                        );
                        workspace_processor.send(GraphsWorkspaceAction::PasteNode {
                            pos,
                            graph_id: graph_state.read().graph_info.id,
                        });
                    }
                    event.stop_propagation();
                } else if event.data().key() == Key::Delete {
                    let nodes_to_delete = graph_store.read().selected_nodes();
                    for node_id in nodes_to_delete.keys() {
                        workspace_processor.send(GraphsWorkspaceAction::DeleteNode {
                            node_id: *node_id,
                            graph_id: graph_state.read().graph_info.id,
                        });
                    }

                    event.stop_propagation();
                }
            }
        }
    }
}

pub fn use_drag_end(workspace: ReadSignal<GraphsWorkspaceState>) -> impl FnMut(MouseEvent) {
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    move |_| {
        let active_graph = workspace.read().active_tab;
        let tabs = workspace.read().tabs.read().clone();
        if let Some(graph_state) = tabs.get(&*active_graph.read()) {
            let editor_status = graph_state.read().editor_state;
            let graph_store = graph_state.read().graph_store;
            let drag_status = workspace.read().drag_status.read().clone();
            let droppable_groups = *workspace.read().drop_in_group.read();
            match drag_status {
                DragStatus::Nodes => {
                    if droppable_groups.is_none() {
                        let selected_nodes = graph_store().selected_nodes();
                        for node_id in selected_nodes.keys() {
                            if let Some(pos) = graph_store
                                .read()
                                .nodes()
                                .read()
                                .get(node_id)
                                .map(NodeElement::pos)
                            {
                                workspace_processor.send(GraphsWorkspaceAction::SyncNodePosition {
                                    pos,
                                    node_id: *node_id,
                                });
                            }
                        }
                    } else if let Some((to_graph_id, _)) = droppable_groups {
                        let selected_optical_nodes = graph_store().selected_optical_nodes();
                        workspace_processor.send(GraphsWorkspaceAction::DropNodesIntoGroup {
                            nodes: selected_optical_nodes.iter().copied().collect(),
                            from_graph_id: *active_graph.read(),
                            to_graph_id,
                        });
                    }
                }
                DragStatus::SelectionBox(_) => {
                    let nodes_to_select = graph_store.read().nodes_to_be_selected();
                    let nodes_to_remove = graph_store.read().nodes_to_be_removed();
                    for (node_id, is_optical) in nodes_to_select {
                        workspace_processor.send(GraphsWorkspaceAction::AddToNodeSelection {
                            graph_id: *active_graph.read(),
                            node_id,
                            is_optical,
                        });
                    }
                    for node_id in nodes_to_remove.keys().copied() {
                        workspace_processor.send(GraphsWorkspaceAction::RemoveFromNodeSelection {
                            graph_id: *active_graph.read(),
                            node_id,
                        });
                    }

                    workspace_processor.send(GraphsWorkspaceAction::ClearNodesToBeRemoved {
                        graph_id: *active_graph.read(),
                    });
                    workspace_processor.send(GraphsWorkspaceAction::ClearNodesToBeSelected {
                        graph_id: *active_graph.read(),
                    });

                    workspace_processor.send(GraphsWorkspaceAction::SetSelectionBox(None));
                }
                DragStatus::Edge(_) => {
                    if let Some(edge) = editor_status.read().edge_in_creation.read().clone()
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
                            graph_id: graph_state.read().graph_info.id,
                        });
                    }
                    workspace_processor.send(GraphsWorkspaceAction::SetEdgeInCreation {
                        graph_id: graph_state.read().graph_info.id,
                        edge_in_creation: None,
                    });
                }
                _ => {}
            }
            workspace_processor.send(GraphsWorkspaceAction::SetDragStatus(DragStatus::None));
        }
    }
}
