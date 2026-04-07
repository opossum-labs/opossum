#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::scenery_editor::{GraphStore, node::Node};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use uuid::Uuid;

#[component]
pub fn Nodes(
    graph_store: ReadSignal<GraphStore>,
    graph_id: Uuid,
    ctrl_pressed: ReadSignal<bool>,
    shift_pressed: ReadSignal<bool>,
    mouse_pos_in_editor: Memo<Point2D<f64>>,
) -> Element {
    rsx! {
        for node in graph_store().nodes().read().iter() {
            {
                rsx! {
                    Node {
                        node: node.1.clone(),
                        ctrl_pressed,
                        shift_pressed,
                        mouse_pos_in_editor,
                    }
                }
            }
        }
    }
}
