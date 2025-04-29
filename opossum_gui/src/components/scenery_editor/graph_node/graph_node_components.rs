use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_backend::usize_to_f64;
use uuid::Uuid;

use crate::components::scenery_editor::DraggedNode;

#[component]
pub fn GraphNodeContent(
    node_header: Element,
    node_body: Element,
    node_size: Point2D<f64>,
) -> Element {
    rsx! {
        div {
            class: "node-content",
            style: format!("width: {}px; height: {}px;", node_size.x, node_size.y),
            {node_header}
            {node_body}
        }
    }
}

#[component]
pub fn GraphNodeHeader(node_name: String, node_id: Uuid, node_size: Point2D<f64>) -> Element {
    let mut dragged_node: Signal<DraggedNode> = use_context::<Signal<DraggedNode>>();

    let font_fac = 6. * usize_to_f64(node_name.len()) / (0.95 * node_size.x);
    let font_size = if font_fac > 1. { 10. / font_fac } else { 10. };
    let header_scale = 0.3;

    let on_drag_start = {
        let id = node_id;
        move |event: Event<MouseData>| {
            event.prevent_default();
            let elem_offset = event.element_coordinates();
            dragged_node.write().set_node_id(id);
            dragged_node
                .write()
                .set_elem_offset((elem_offset.x, elem_offset.y));
        }
    };

    rsx! {
        div {
            onmousedown: on_drag_start,
            class: "node-header",
            style: format!("height: {}px;font-size: {font_size}pt;", node_size.y * header_scale),
            {node_name}
        }
    }
}
