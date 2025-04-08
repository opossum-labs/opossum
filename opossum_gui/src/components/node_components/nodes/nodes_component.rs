use dioxus::prelude::*;

use crate::{components::node_components::Node, NODES_STORE};

#[component]
pub fn Nodes() -> Element {
    rsx! {
        for node in NODES_STORE.read().optic_nodes().read().iter() {
            {
                rsx! {
                    Node { node: node.clone() }
                }
            }
        }
    }
}
