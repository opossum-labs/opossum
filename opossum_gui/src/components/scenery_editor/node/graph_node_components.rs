use super::NodeType;
use crate::components::scenery_editor::constants::BORDER_WIDTH;
use crate::components::scenery_editor::node::{HEADER_HEIGHT, NODE_WIDTH};
use dioxus::prelude::*;
use opossum_core::utils::to_f64;

/// Renders a canvas node as header + body (+ an optional footer below the body).
///
/// The footer is a sibling of the body rather than part of it, because the ports live *inside*
/// `.node-body` - anything appended below therefore grows the node without moving a single port,
/// and edges stay where they are.
#[component]
pub fn GraphNodeContent(
    name: String,
    node_type: NodeType,
    body: Element,
    footer: Option<Element>,
) -> Element {
    let node_type = match node_type {
        NodeType::Optical(_) => "optic-node",
        NodeType::Analyzer(_) => "analyzer-node",
    };
    let font_fac = 6. * to_f64(name.len()) / (0.95 * NODE_WIDTH);
    let font_size = if font_fac > 1. { 10. / font_fac } else { 10. };
    rsx! {
        div {
            class: "node-header {node_type}",
            pointer_events: "none",
            style: format!(
                "width: {NODE_WIDTH}px; height: {HEADER_HEIGHT}px; font-size: {font_size}pt; border-bottom-width:{BORDER_WIDTH}px",
            ),
            {name}
        }
        div { draggable: false, {body} }
        if let Some(footer) = footer {
            {footer}
        }
    }
}
