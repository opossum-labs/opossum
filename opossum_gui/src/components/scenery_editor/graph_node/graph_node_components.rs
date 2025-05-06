use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_backend::usize_to_f64;

#[component]
pub fn GraphNodeContent(node_name: String, node_body: Element, node_size: Point2D<f64>) -> Element {
    rsx! {
        div {
            class: "node-content",
            style: format!("width: {}px; height: {}px;", node_size.x, node_size.y),
            GraphNodeHeader { node_name, node_size }
            {node_body}
        }
    }
}

#[component]
pub fn GraphNodeHeader(node_name: String, node_size: Point2D<f64>) -> Element {
    let font_fac = 6. * usize_to_f64(node_name.len()) / (0.95 * node_size.x);
    let font_size = if font_fac > 1. { 10. / font_fac } else { 10. };
    let header_scale = 0.3;

    rsx! {
        div {
            class: "node-header",
            style: format!("height: {}px;font-size: {font_size}pt;", node_size.y * header_scale),
            {node_name}
        }
    }
}
