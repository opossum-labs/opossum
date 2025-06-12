use super::NodeType;
use crate::components::scenery_editor::node::{HEADER_HEIGHT, NODE_WIDTH};
use dioxus::prelude::*;
use opossum_backend::usize_to_f64;

#[component]
pub fn GraphNodeContent(node_name: String, node_type: NodeType, node_body: Element) -> Element {
    let node_type = match node_type {
        NodeType::Optical(_) => "optic-node",
        NodeType::Analyzer(_) => "analyzer-node",
    };
    let font_fac = 6. * usize_to_f64(node_name.len()) / (0.95 * NODE_WIDTH);
    let font_size = if font_fac > 1. { 10. / font_fac } else { 10. };
    rsx! {
        div {
            class: "node-header {node_type}",
            pointer_events: "none",
            style: format!(
                "width: {NODE_WIDTH}px; height: {HEADER_HEIGHT}px; font-size: {font_size}pt;",
            ),
            {node_name}
        }
        div { draggable: false, {node_body} }
    }
}
