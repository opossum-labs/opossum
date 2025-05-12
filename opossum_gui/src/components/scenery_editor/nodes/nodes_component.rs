use dioxus::prelude::*;
use uuid::Uuid;

use crate::components::scenery_editor::{graph_store::GraphStore, node::Node};

#[component]
pub fn Nodes(node_activated: Signal<Option<Uuid>>) -> Element {
    let nodes_store = use_context::<GraphStore>();
    rsx! {
        for node in nodes_store.nodes().read().iter() {
            {
                rsx! {
                    Node { node: node.1.clone(), node_activated: node_activated.clone() }
                }
            }
        }
    }
}
