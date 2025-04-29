use dioxus::prelude::*;

use crate::components::scenery_editor::{Node, NODES_STORE};

#[component]
pub fn Nodes() -> Element {
    let node_activated = use_signal(|| false);
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
