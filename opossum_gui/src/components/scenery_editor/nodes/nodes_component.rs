#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::scenery_editor::{
    graph_store::GraphStore,
    node::{Node, NodeElement},
};
use dioxus::prelude::*;

#[component]
pub fn Nodes(node_activated: Signal<Option<NodeElement>>) -> Element {
    let nodes_store = use_context::<GraphStore>();
    rsx! {
        for node in nodes_store.nodes().read().iter() {
            {
                rsx! {
                    Node { node: node.1.clone(), node_activated }
                }
            }
        }
    }
}
