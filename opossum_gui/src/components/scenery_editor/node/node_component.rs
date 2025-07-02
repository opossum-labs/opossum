#![allow(clippy::derive_partial_eq_without_eq)]
use super::NodeElement;
use crate::components::context_menu::cx_menu::CxMenu;
use crate::components::context_menu::cx_menu::CxtCommand;
use crate::components::scenery_editor::graph_store::GraphStoreAction;
use crate::components::scenery_editor::{
    graph_editor::graph_editor_component::{DragStatus, EditorState},
    graph_store::GraphStore,
    node::graph_node_components::GraphNodeContent,
    ports::ports_component::NodePorts,
};
use crate::CONTEXT_MENU;
use dioxus::prelude::*;
use opossum_backend::nodes::NewRefNode;

#[component]
pub fn Node(node: NodeElement, node_activated: Signal<Option<NodeElement>>) -> Element {
    let mut editor_status = use_context::<EditorState>();
    let graph_store = use_context::<Signal<GraphStore>>();
    let graph_processor = use_context::<Coroutine<GraphStoreAction>>();
    let position = node.pos();
    let active_node_id = graph_store().active_node();
    let is_active = active_node_id.map_or("", |active_node_id| {
        if active_node_id == node.id() {
            "active-node"
        } else {
            ""
        }
    });
    let id = node.id();
    let z_index = if let Some(active_node_id) = active_node_id {
        if active_node_id == node.id {
            9999
        } else {
            node.z_index()
        }
    } else {
        node.z_index()
    };
    let node_icon = node.node_type.icon();
    rsx! {
        div {
            tabindex: 0, // necessary to allow to receive keyboard focus
            class: "node {is_active}",
            draggable: false,
            style: format!("left: {}px; top: {}px; z-index: {z_index};", position.x, position.y),
            onmousedown: move |event: MouseEvent| {
                editor_status.drag_status.set(DragStatus::Node(id));
                let previously_selected = graph_store().active_node();
                if previously_selected != Some(id) {
                    graph_store().set_node_active(id);
                    node_activated.set(Some(node.clone()));
                }
                event.stop_propagation();
            },
            onkeydown: move |event| {
                if event.data().key() == Key::Delete {
                    graph_processor.send(GraphStoreAction::DeleteNode(id));
                }
                event.stop_propagation();
            },
            oncontextmenu: {
                move |event: Event<MouseData>| {
                    event.prevent_default();
                    let new_ref_node = NewRefNode::new(
                        id,
                        (event.page_coordinates().x, event.page_coordinates().y),
                    );
                    let cx_menu = CxMenu::new(
                        event.page_coordinates().x,
                        event.page_coordinates().y,
                        vec![
                            ("Create reference".to_owned(), CxtCommand::AddRefNode(new_ref_node)),
                        ],
                    );
                    println!("oncontext: {cx_menu:?}");
                    let mut ctx = CONTEXT_MENU.write();
                    *ctx = cx_menu;
                }
            },
            GraphNodeContent {
                node_name: node.name(),
                node_type: node.node_type().clone(),
                node_body: rsx! {
                    div {
                        class: "node-body",
                        draggable: false,
                        style: format!("height: {}px;", node.node_body_height()),
                        if node_icon.is_some() {
                            img {
                                src: node_icon.unwrap(),
                                width: "50px",
                                style: "display: block; margin: auto;",
                                draggable: false,
                            }
                        }
                        NodePorts { node: node.clone() }
                    }
                },
            }
        }
    }
}
