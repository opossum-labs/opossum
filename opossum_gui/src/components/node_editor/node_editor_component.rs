use dioxus::prelude::*;

use crate::components::scenery_editor::node::NodeElement;

#[component]
pub fn NodeEditor(node: ReadOnlySignal<Option<NodeElement>>) -> Element {
    let selected_node = node.read_unchecked();
    selected_node.clone().map_or_else(
        || {
            rsx! {
                div { "No node selected" }
            }
        },
        |node_element| {
            rsx!(
                div {
                    h5 { "Node Properties" }
                    p { {format!("ID: {}", node_element.id())} }
                                // Add more properties here as needed
                }
            )
        },
    )
}
