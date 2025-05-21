#![allow(clippy::derive_partial_eq_without_eq)]
use super::NodeElement;
use crate::components::scenery_editor::{
    graph_editor::graph_editor_component::{DragStatus, EditorState},
    graph_store::GraphStore,
    node::graph_node_components::GraphNodeContent,
    ports::ports_component::NodePorts,
};
use dioxus::prelude::*;

#[component]
pub fn Node(node: NodeElement, node_activated: Signal<Option<NodeElement>>) -> Element {
    let mut editor_status = use_context::<EditorState>();
    let mut graph_store = use_context::<GraphStore>();
    let position = node.pos();
    let active_node_id = graph_store.active_node();
    let is_active = active_node_id.map_or("", |active_node_id| {
        if active_node_id == node.id() {
            "active-node"
        } else {
            ""
        }
    });
    let id = node.id();
    let z_index = node.z_index();
    rsx! {
        div {
            tabindex: 0, // necessary to allow to receive keyboard focus
            class: "node {is_active}",
            draggable: false,
            style: format!("left: {}px; top: {}px; z-index: {z_index};", position.x, position.y),
            onmousedown: move |event: MouseEvent| {
                editor_status.drag_status.set(DragStatus::Node(id));
                let previously_selected = graph_store.active_node();
                if previously_selected != Some(id) {
                    graph_store.set_node_active(id);
                    node_activated.set(Some(node.clone()));
                }
                event.stop_propagation();
            },
            onkeydown: move |event| {
                if event.data().key() == Key::Delete {
                    spawn(async move { graph_store.delete_node(id).await });
                }
                event.stop_propagation();
            },
            GraphNodeContent {
                node_name: node.name(),
                node_type: node.node_type().clone(),
                node_body: rsx! {
                    div {
                        class: "node-body",
                        draggable: false,
                        style: format!("height: {}px;", node.node_body_height()),
                        NodePorts { node: node.clone() }
                    }
                },
            }
        }
    }
}
// #[must_use]
// fn use_node_context_menu(node_id: Uuid) -> Callback<Event<MouseData>> {
//     use_callback(move |evt: Event<MouseData>| {
//         println!("Node context menu clicked");
//         evt.prevent_default();
//         let mut cx_menu = CONTEXT_MENU.write();
//         *cx_menu = CxMenu::new(
//             evt.page_coordinates().x,
//             evt.page_coordinates().y,
//             vec![("Delete node".to_owned(), use_delete_node(node_id))],
//         );
//     })
// }
// #[must_use]
// fn use_delete_node(node_id: Uuid) -> Callback<Event<MouseData>> {
//     use_callback(move |_: Event<MouseData>| {
//         let node_id = node_id;
//         spawn(async move {
//             match api::delete_node(&HTTP_API_CLIENT(), node_id).await {
//                 Ok(_id_vec) => {
//                     // for id in &id_vec {
//                     //    graph_store.delete_node(*id);
//                     // }
//                 }
//                 Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
//             }
//         });
//     })
// }
