use crate::components::scenery_editor::{
    constants::{MAX_ZOOM, MIN_ZOOM, ZOOM_SENSITIVITY},
    edges::edges_component::EdgeCreation,
    graph_editor::graph_editor_component::{DragStatus, EditorState, ShiftZoom},
    graph_store::{GraphStore, GraphStoreAction},
};
use dioxus::{
    html::geometry::{euclid::default::Point2D, PixelsSize},
    prelude::*,
};
use opossum_backend::{nodes::ConnectInfo, PortType};

pub fn use_zoom(
    mut graph_shift_zoom: Signal<ShiftZoom>,
    on_mounted: Signal<Option<std::rc::Rc<MountedData>>>,
) -> impl FnMut(WheelEvent) {
    move |wheel_event| {
        spawn(async move {
            if let Ok(rect) = on_mounted().unwrap().get_client_rect().await {
                let client_pos = wheel_event.data.client_coordinates();
                let mouse_pos =
                    Point2D::new(client_pos.x - rect.min_x(), client_pos.y - rect.min_y());
                let current_graph_shift = graph_shift_zoom.read().shift;
                let current_graph_zoom = graph_shift_zoom.read().zoom;
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
                graph_shift_zoom.set(ShiftZoom::new(
                    new_graph_zoom,
                    Point2D::new(new_shift_x, new_shift_y),
                ));
            }
        });
    }
}

pub fn use_center_graph(
    graph_store: Signal<GraphStore>,
    editor_size: Signal<Option<PixelsSize>>,
    mut graph_shift_zoom: Signal<ShiftZoom>,
) -> impl FnMut(MouseEvent) {
    move |mouse_event| {
        mouse_event.stop_propagation();
        let bounding_box = graph_store().get_bounding_box();
        let center = bounding_box.center();
        if let Some(window_size) = editor_size() {
            let zoom = graph_shift_zoom.read().zoom;
            let view_center_x = window_size.width / 2.0;
            let view_center_y = window_size.height / 2.0;
            graph_shift_zoom.set(ShiftZoom::new(
                zoom,
                Point2D::new(
                    center.x.mul_add(-zoom, view_center_x),
                    center.y.mul_add(-zoom, view_center_y),
                ),
            ));
        }
    }
}
pub fn use_drag_start(
    mut editor_status: EditorState,
    mut current_mouse_pos: Signal<Point2D<f64>>,
) -> impl FnMut(MouseEvent) {
    move |event| {
        current_mouse_pos.set(Point2D::new(
            event.client_coordinates().x,
            event.client_coordinates().y,
        ));
        editor_status.drag_status.set(DragStatus::Graph);
    }
}
pub fn use_drag(
    mut editor_status: EditorState,
    mut current_mouse_pos: Signal<Point2D<f64>>,
    mut graph_shift_zoom: Signal<ShiftZoom>,
    graph_store: Signal<GraphStore>,
) -> impl FnMut(MouseEvent) {
    move |event| {
        let current_sz = *graph_shift_zoom.read();
        let drag_status = editor_status.drag_status.read().clone();
        let rel_shift_x = event.client_coordinates().x - current_mouse_pos().x;
        let rel_shift_y = event.client_coordinates().y - current_mouse_pos().y;
        current_mouse_pos.set(Point2D::new(
            event.client_coordinates().x,
            event.client_coordinates().y,
        ));
        let graph_shift =
            Point2D::new(rel_shift_x / current_sz.zoom, rel_shift_y / current_sz.zoom);
        match drag_status {
            DragStatus::Graph => {
                let shift = current_sz.shift;
                graph_shift_zoom.set(ShiftZoom::new(
                    current_sz.zoom,
                    Point2D::new(shift.x + rel_shift_x, shift.y + rel_shift_y),
                ));
            }
            DragStatus::Node(id) => {
                graph_store().shift_node_position(id, graph_shift);
            }
            DragStatus::Edge(edge_creation_start) => {
                editor_status.edge_in_creation.with_mut(|edge_option| {
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
pub fn use_drag_end(
    mut editor_status: EditorState,
    graph_processor: Coroutine<GraphStoreAction>,
) -> impl FnMut(MouseEvent) {
    move |_| {
        let drag_status = editor_status.drag_status.read().clone();
        match drag_status {
            DragStatus::Node(uuid) => {
                graph_processor.send(GraphStoreAction::SyncNodePosition(uuid));
            }
            DragStatus::Edge(_) => {
                if let Some(edge) = editor_status.edge_in_creation.write().take() {
                    if edge.is_valid() {
                        if let (Some(end_port), start_port) = (edge.end_port(), edge.start_port()) {
                            let (start_port, end_port) = if start_port.port_type == PortType::Output
                            {
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
                }
            }
            _ => {}
        }
        editor_status.drag_status.set(DragStatus::None);
    }
}
