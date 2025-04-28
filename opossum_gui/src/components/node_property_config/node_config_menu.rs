use crate::ACTIVE_NODE;
use dioxus::prelude::*;

#[component]
pub fn NodePropertyConfigMenu() -> Element {
    ACTIVE_NODE().map_or_else(
        || {
            rsx! {
                div { class: "property-config-window", "No node selected" }
            }
        },
        |node_attr| {
            rsx!(
                div { class: "property-config-window",
                    h5 { "Node Properties" }
                    p { {format!("Node Name: {}", node_attr.name())} }
                                // Add more properties here as needed
                }
            )
        },
    )
}
