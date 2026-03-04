#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::scenery_editor::{graph_store::GraphStore, node::Node};
use dioxus::prelude::*;
use uuid::Uuid;

#[component]
pub fn Nodes(graph_store: Signal<GraphStore>, graph_id: Uuid) -> Element {
    rsx! {
        for node in graph_store().nodes().read().iter() {
            {
                rsx! {
                    Node { node: node.1.clone() }
                }
            }
        }
    }
}
