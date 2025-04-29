use dioxus::prelude::*;
use uuid::Uuid;

#[component]
pub fn NodeEditor(node: Signal<Option<Uuid>>) -> Element {
    let selected_node=node.read_unchecked();
    selected_node.map_or_else(
        || {
            rsx! {
                div {"No node selected" }
            }
        },
        |uuid| {
            rsx!(
                div {
                    h5 { "Node Properties" }
                    p { {format!("ID: {}", uuid)} }
                    // Add more properties here as needed
                }
            )
        },
    )
}
