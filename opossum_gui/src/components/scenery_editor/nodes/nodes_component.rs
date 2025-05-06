use dioxus::prelude::*;
use uuid::Uuid;

use crate::components::scenery_editor::{node::Node, nodes::NodesStore};

#[component]
pub fn Nodes(node_activated: Signal<Option<Uuid>>) -> Element {
    let nodes_store = use_context::<NodesStore>();
    rsx! {
        for node in nodes_store.optic_nodes().read().iter() {
            {
                rsx! {
                    Node { node: node.1.clone(), node_activated: node_activated.clone() }
                }
            }
        }
    }
}
