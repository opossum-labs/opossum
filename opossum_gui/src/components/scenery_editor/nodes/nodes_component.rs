#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::scenery_editor::{graph_store::GraphStore, node::Node};
use dioxus::prelude::*;
use uuid::Uuid;

#[component]
pub fn Nodes(add_new_group_tab_handler: EventHandler<(String, Uuid)>) -> Element {
    let graph_store = use_context::<Signal<GraphStore>>();
    rsx! {
        for node in graph_store().nodes().read().iter() {
            {
                rsx! {
                    Node { node: node.1.clone(), add_new_group_tab_handler }
                }
            }
        }
    }
}
