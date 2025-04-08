use dioxus::prelude::*;

use crate::ACTIVE_NODE;

#[component]
pub fn NodePropertyConfigMenu() -> Element {
    if let Some(node_attr) = ACTIVE_NODE().clone() {
        rsx!(
            div { class: "property-config-window",
                h3 { "Node Properties" }
                p { {format!("Node Name: {}", node_attr.name())} }
                        // Add more properties here as needed
            }
        )
    } else {
        rsx! {
            div { class: "property-config-window", "No node selected" }
        }
    }
}
