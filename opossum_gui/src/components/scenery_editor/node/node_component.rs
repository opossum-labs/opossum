#![allow(clippy::derive_partial_eq_without_eq)]
use super::NodeElement;
use crate::components::scenery_editor::{
    graph_editor::graph_editor_component::{DragStatus, EditorState},
    graph_node::graph_node_components::GraphNodeContent,
    nodes::NodesStore,
    ports::ports_component::NodePorts,
};
use dioxus::prelude::*;
use uuid::Uuid;

#[component]
pub fn Node(node: NodeElement, node_activated: Signal<Option<Uuid>>) -> Element {
    let mut editor_status = use_context::<EditorState>();
    let mut node_store = use_context::<NodesStore>();
    let position = node.pos();
    let active_node_id = node_store.active_node();
    let is_active = if let Some(active_node_id) = active_node_id {
        if active_node_id == node.id() {
            "active-node"
        } else {
            ""
        }
    } else {
        ""
    };
    let id = node.id();
    let z_index = node.z_index();
    rsx! {
        div {
            class: "node {is_active}",
            draggable: false,
            style: format!(
                "left: {}px; top: {}px; z-index: {z_index};",
                position.x as i32,
                position.y as i32,
            ),
            onmousedown: move |event: MouseEvent| {
                editor_status.drag_status.set(DragStatus::Node(id));
                node_store.set_node_active(id, z_index);
                node_activated.set(Some(id));
                event.stop_propagation();
            },
            //oncontextmenu: use_node_context_menu(*node.id()),

            GraphNodeContent {
                node_name: node.name(),
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
//                     //    node_store.delete_node(*id);
//                     // }
//                 }
//                 Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
//             }
//         });
//     })
// }
