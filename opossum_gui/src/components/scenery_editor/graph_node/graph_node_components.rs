use crate::components::scenery_editor::node::{HEADER_HEIGHT, NODE_WIDTH};
use dioxus::prelude::*;
use opossum_backend::usize_to_f64;

#[component]
pub fn GraphNodeContent(node_name: String, node_body: Element) -> Element {
    rsx! {
        div { draggable: false,
            GraphNodeHeader { node_name }
            {node_body}
        }
    }
}

#[component]
pub fn GraphNodeHeader(node_name: String) -> Element {
    let font_fac = 6. * usize_to_f64(node_name.len()) / (0.95 * NODE_WIDTH);
    let font_size = if font_fac > 1. { 10. / font_fac } else { 10. };
    rsx! {
        div {
            class: "node-header",
            style: format!(
                "width: {NODE_WIDTH}px; height: {HEADER_HEIGHT}px; font-size: {font_size}pt;",
            ),
            {node_name}
        }
    }
}
