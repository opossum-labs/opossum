use dioxus::prelude::*;
use uuid::Uuid;
use crate::components::scenery_editor::{Node, NODES_STORE};

#[component]
pub fn Nodes(node_activated: Signal<Option<Uuid>>) -> Element {
    rsx! {
        for node in NODES_STORE.read().optic_nodes().read().iter() {
            {
                rsx! {
                    Node { node: node.clone(), node_activated: node_activated.clone() }
                }
            }
        }
    }
}
