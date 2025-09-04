use std::rc::Rc;

use crate::components::scenery_editor::{
    NodeElement, NodeType,
    constants::{MAX_ZOOM, MIN_ZOOM, ZOOM_SENSITIVITY},
    edges::edges_component::EdgeCreation,
    graph_editor::graph_editor_component::{DragStatus, EditorState},
    graph_store::{GraphStore, GraphStoreAction},
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_backend::{PortType, nodes::ConnectInfo};
use uuid::Uuid;

pub fn use_zoom(on_mounted: Signal<Option<std::rc::Rc<MountedData>>>) -> impl FnMut(WheelEvent) {
    let editor_status = use_context::<Signal<EditorState>>();

    move |wheel_event| {
        let mut zoom = editor_status().zoom;
        let mut shift = editor_status().shift;
        spawn(async move {
            if let Ok(rect) = on_mounted().unwrap().get_client_rect().await {
                let client_pos = wheel_event.data.client_coordinates();
                let mouse_pos =
                    Point2D::new(client_pos.x - rect.min_x(), client_pos.y - rect.min_y());
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
        });
    }
}

pub fn use_center_graph() -> impl FnMut(MouseEvent) {
    let graph_store = use_context::<Signal<GraphStore>>();
    let mut editor_status = use_context::<Signal<EditorState>>();

    move |mouse_event| {
        mouse_event.stop_propagation();
        let bounding_box = graph_store().get_bounding_box();
        let center = bounding_box.center();
        let zoom = *editor_status.read().zoom.read();
        let view_center = editor_status.read().get_view_port_center();
        editor_status.write().shift.set(Point2D::new(
            center.x.mul_add(-zoom, view_center.x),
            center.y.mul_add(-zoom, view_center.y),
        ));
    }
}
pub fn use_drag_start(
    mut current_mouse_pos: Signal<Point2D<f64>>,
    mut node_selected: Signal<Option<NodeElement>>,
) -> impl FnMut(MouseEvent) {
    let mut editor_status = use_context::<Signal<EditorState>>();

    move |event| {
        node_selected.set(None);
        current_mouse_pos.set(Point2D::new(
            event.client_coordinates().x,
            event.client_coordinates().y,
        ));
        editor_status.write().drag_status.set(DragStatus::Graph);
    }
}
pub fn use_drag(mut current_mouse_pos: Signal<Point2D<f64>>) -> impl FnMut(MouseEvent) {
    let mut editor_status = use_context::<Signal<EditorState>>();
    let graph_store = use_context::<Signal<GraphStore>>();
    let current_shift = *editor_status().shift.read();
    let current_zoom = *editor_status().zoom.read();

    move |event| {
        let drag_status = editor_status.read().drag_status.read().clone();
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
            DragStatus::Node(id) => {
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
            DragStatus::None => {}
        }
    }
}

pub fn use_on_resize(on_mounted: Signal<Option<Rc<MountedData>>>) -> impl FnMut(ResizeEvent) {
    let mut editor_status = use_context::<Signal<EditorState>>();

    move |event| {
        if let Ok(size) = event.data().get_content_box_size() {
            editor_status.write().editor_size.set(size);
        }
        spawn(async move {
            if let Ok(rect) = on_mounted().unwrap().get_client_rect().await {
                editor_status.write().rect.set(rect);
            }
        });
    }
}

pub fn use_on_key_down(
    mouse_pos: Signal<Point2D<f64>>,
    node_selected: Signal<Option<NodeElement>>,
    mut copied_node: Signal<Option<(NodeType, Uuid)>>,
    mouse_inside: Signal<bool>,
) -> impl FnMut(KeyboardEvent) {
    let editor_status = use_context::<Signal<EditorState>>();
    let graph_processor = use_coroutine_handle::<GraphStoreAction>();
    move |event| {
        if !event.is_auto_repeating() {
            let modifiers = event.modifiers();
            let ctrl_or_meta = modifiers.ctrl() || modifiers.meta();
            if ctrl_or_meta
                && event.data().key() == Key::Character("c".to_string())
                && let Some(node) = &*node_selected.peek()
            {
                copied_node.set(Some((node.node_type().clone(), node.id())));
            }
            if ctrl_or_meta
                && event.data().key() == Key::Character("v".to_string())
                && let Some((node_type, node_id)) = &*copied_node.read()
            {
                let rect = *editor_status().rect.read();
                let mouse = *mouse_pos.read();
                if *mouse_inside.read() {
                    let shift = *editor_status().shift.read();
                    let zoom = *editor_status().zoom.read();
                    let pos = Point2D::new(
                        (mouse.x - shift.x - rect.min_x()) / zoom,
                        (mouse.y - shift.y - rect.min_y()) / zoom,
                    );
                    graph_processor.send(GraphStoreAction::CopyNode((
                        node_type.clone(),
                        *node_id,
                        pos,
                    )));
                }
            }
        }
        event.stop_propagation();
    }
}

pub fn use_on_mouse_leave(mut mouse_inside_sig: Signal<bool>) -> impl FnMut(MouseEvent) {
    move |_| {
        let mut editor_status = use_context::<Signal<EditorState>>();
        mouse_inside_sig.set(false);
        editor_status.write().drag_status.set(DragStatus::None);
    }
}

pub fn use_drag_end() -> impl FnMut(MouseEvent) {
    let graph_store = use_context::<Signal<GraphStore>>();
    let mut editor_status = use_context::<Signal<EditorState>>();
    let graph_processor = use_coroutine_handle::<GraphStoreAction>();
    move |_| {
        let drag_status = editor_status.read().drag_status.read().clone();
        match drag_status {
            DragStatus::Node(uuid) => {
                if let Some(pos) = graph_store
                    .read()
                    .nodes()
                    .read()
                    .get(&uuid)
                    .map(NodeElement::pos)
                {
                    graph_processor.send(GraphStoreAction::SyncNodePosition(uuid, pos));
                }
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
                    );
                    graph_processor.send(GraphStoreAction::AddEdge(new_edge));
                }
            }
            _ => {}
        }
        editor_status.write().drag_status.set(DragStatus::None);
    }
}
