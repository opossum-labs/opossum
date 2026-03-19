use std::time::{Duration, Instant};

use crate::{
    CONTEXT_MENU,
    components::scenery_editor::{
        NodeElement,
        constants::{MAX_ZOOM, MIN_ZOOM, ZOOM_SENSITIVITY},
        edges::edges_component::EdgeCreation,
        graph_editor::graph_workspace::{
            DragStatus, EditorState, GraphStore, GraphsWorkspaceAction, GraphsWorkspaceState,
            WorkSpaceSignalHandlers,
        },
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
use serde_json::Value;

pub fn use_zoom() -> impl FnMut(WheelEvent) {
    let editor_status = use_context::<Signal<EditorState>>();
    let workspace = use_context::<Signal<GraphsWorkspaceState>>();

    move |wheel_event| {
        let mut zoom = editor_status().zoom;
        let mut shift = editor_status().shift;
        let rect = *workspace.read().editor_rect.read();
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
        zoom.set(new_graph_zoom);
        shift.set(Point2D::new(new_shift_x, new_shift_y));
    }
}

pub fn use_on_mouse_down(
    mut current_mouse_pos: Signal<Point2D<f64>>,
    mut last_click: Signal<Option<Instant>>,
) -> impl FnMut(MouseEvent) {
    let dc_time = Duration::from_millis(300);
    let editor_status = use_context::<Signal<EditorState>>();
    let workspace_handlers = use_context::<WorkSpaceSignalHandlers>();
    let mut workspace = use_context::<Signal<GraphsWorkspaceState>>();

    move |event: MouseEvent| {
        event.stop_propagation();
        if let Some(trigger_button) = event.trigger_button() {
            match trigger_button {
                MouseButton::Primary => {
                    let mut ctx = CONTEXT_MENU.write();
                    *ctx = None;

                    let mouse_pos =
                        Point2D::new(event.client_coordinates().x, event.client_coordinates().y);

                    let editor_origin = workspace().editor_rect.read().origin;
                    let current_shift = *editor_status().shift.read();
                    let current_zoom = *editor_status().zoom.read();

                    let graph_origin = Point2D::new(
                        (mouse_pos.x - editor_origin.x - current_shift.x) / current_zoom,
                        (mouse_pos.y - editor_origin.y - current_shift.y) / current_zoom,
                    );

                    workspace
                        .write()
                        .drag_status
                        .set(DragStatus::SelectionBox(Rect::new(
                            graph_origin,
                            Size2D::new(0., 0.),
                        )));
                }
                MouseButton::Auxiliary => {
                    //for dragging
                    current_mouse_pos.set(Point2D::new(
                        event.client_coordinates().x,
                        event.client_coordinates().y,
                    ));
                    workspace.write().drag_status.set(DragStatus::Graph);

                    //for double-click zoom
                    event.stop_propagation();
                    let now = Instant::now();
                    let t0_opt = *last_click.read();
                    if let Some(t0) = t0_opt
                        && now.duration_since(t0) < dc_time
                    {
                        workspace_handlers
                            .view
                            .center_graph(*workspace.read().active_tab.read(), true);
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
    let mut editor_status = use_context::<Signal<EditorState>>();
    let graph_store = use_context::<Signal<GraphStore>>();
    let mut workspace = use_context::<Signal<GraphsWorkspaceState>>();

    move |event| {
        let current_shift = *editor_status().shift.read();
        let current_zoom = *editor_status().zoom.read();
        let drag_status = workspace.read().drag_status.read().clone();
        let rel_shift_x = event.client_coordinates().x - current_mouse_pos().x;
        let rel_shift_y = event.client_coordinates().y - current_mouse_pos().y;
        current_mouse_pos.set(Point2D::new(
            event.client_coordinates().x,
            event.client_coordinates().y,
        ));
        let graph_shift = Point2D::new(rel_shift_x / current_zoom, rel_shift_y / current_zoom);
        match drag_status {
            DragStatus::Graph => {
                editor_status.write().shift.set(Point2D::new(
                    current_shift.x + rel_shift_x,
                    current_shift.y + rel_shift_y,
                ));
            }
            DragStatus::Node(id, _) => {
                graph_store().shift_node_position(id, graph_shift);
            }
            DragStatus::Edge(edge_creation_start) => {
                editor_status
                    .write()
                    .edge_in_creation
                    .with_mut(|edge_option| {
                        let edge = edge_option.get_or_insert_with(|| {
                            EdgeCreation::new(
                                edge_creation_start.src_node,
                                edge_creation_start.src_port.clone(),
                                edge_creation_start.src_port_type.clone(),
                                edge_creation_start.start_pos,
                            )
                        });
                        edge.shift_end(graph_shift);
                    });
            }
            DragStatus::SelectionBox(rect) => {
                let mouse_pos =
                    Point2D::new(event.client_coordinates().x, event.client_coordinates().y);

                let editor_origin = workspace.read().editor_rect.read().origin;
                let current_shift = *editor_status().shift.read();
                let current_zoom = *editor_status().zoom.read();

                let graph_pos = Point2D::new(
                    (mouse_pos.x - editor_origin.x - current_shift.x) / current_zoom,
                    (mouse_pos.y - editor_origin.y - current_shift.y) / current_zoom,
                );

                let width = graph_pos.x - rect.origin.x;
                let height = graph_pos.y - rect.origin.y;

                let new_rect_orig_x = if width < 0. {
                    graph_pos.x
                } else {
                    rect.origin.x
                };
                let new_rect_orig_y = if height < 0. {
                    graph_pos.y
                } else {
                    rect.origin.y
                };

                let new_rect = Rect::new(
                    Point2D::new(new_rect_orig_x, new_rect_orig_y),
                    Size2D::new(width.abs(), height.abs()),
                );

                workspace.write().selection_box.set(Some(new_rect));
            }
            DragStatus::None => {}
        }
    }
}

pub fn use_on_resize(
    mut workspace: Signal<GraphsWorkspaceState>,
    element_id: String,
) -> EventHandler<()> {
    EventHandler::new(move |()| {
        let element_id = if let Some((graph_id)) = workspace.read().tabs.read().keys().next(){
            format!("editor_{}", graph_id.as_simple())
        }
        else{
            element_id.clone()
        };
        spawn({
            async move {
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
                if let Ok(rect) = eval.recv::<Value>().await {
                    let x = rect["x"].as_f64().unwrap();
                    let y = rect["y"].as_f64().unwrap();
                    let width = rect["width"].as_f64().unwrap();
                    let height = rect["height"].as_f64().unwrap();
                    workspace
                        .write()
                        .editor_rect
                        .set(Rect::new(Point2D::new(x, y), Size2D::new(width, height)));
                }
            }
        });
    })
}

pub fn use_on_key_down(
    mouse_pos: Signal<Point2D<f64>>,
    workspace: Signal<GraphsWorkspaceState>,
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
                if ctrl_or_meta
                    && !modifiers.shift()
                    && event.data().key() == Key::Character("c".to_string())
                    && let Some(node) = graph_store.read().get_active_node()
                {
                    workspace_processor.send(GraphsWorkspaceAction::CopyNode {
                        node_type: node.node_type().clone(),
                        node_id: node.id(),
                    });
                    event.stop_propagation();
                } else if ctrl_or_meta
                    && !modifiers.shift()
                    && event.data().key() == Key::Character("v".to_string())
                {
                    let rect = *workspace().editor_rect.read();
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
                }
            }
        }
    }
}

pub fn use_drag_end(mut workspace: Signal<GraphsWorkspaceState>) -> impl FnMut(MouseEvent) {
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();

    move |_| {
        let active_graph = workspace.read().active_tab;
        let tabs = workspace.read().tabs.read().clone();
        if let Some(graph_state) = tabs.get(&*active_graph.read()) {
            let mut editor_status = graph_state.read().editor_state;
            let graph_store = graph_state.read().graph_store;
            let drag_status = workspace.read().drag_status.read().clone();
            match drag_status {
                DragStatus::Node(node_id, old_position) => {
                    if let Some(pos) = graph_store
                        .read()
                        .nodes()
                        .read()
                        .get(&node_id)
                        .map(NodeElement::pos)
                    {
                        // Update node GUI position (only if really changed)
                        if pos != old_position {
                            workspace_processor
                                .send(GraphsWorkspaceAction::SyncNodePosition { pos, node_id });
                        }
                    }
                }
                DragStatus::SelectionBox(_) => {
                    workspace.write().selection_box.set(None);
                }
                DragStatus::Edge(_) => {
                    if let Some(edge) = editor_status.write().edge_in_creation.write().take()
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
                }
                _ => {}
            }
            workspace.write().drag_status.set(DragStatus::None);
        }
    }
}
