#![allow(clippy::derive_partial_eq_without_eq)]
use super::NodeElement;
use crate::{
    api::{self},
    components::{
        context_menu::cx_menu::CxMenu,
        scenery_editor::{
            graph_editor::graph_editor_component::{DragStatus, EditorState},
            graph_node::graph_node_components::{GraphNodeContent, GraphNodeHeader},
            nodes::NodesStore,
            ports::ports_component::NodePorts,
        },
    },
    CONTEXT_MENU, HTTP_API_CLIENT, OPOSSUM_UI_LOGS,
};
use dioxus::prelude::*;
use opossum_backend::usize_to_f64;
use uuid::Uuid;

#[component]
pub fn Node(node: NodeElement, node_activated: Signal<Option<Uuid>>) -> Element {
    let mut editor_status = use_context::<EditorState>();
    let mut node_store = use_context::<NodesStore>();
    let mut shift = use_signal(|| (0, 0));

    let mut current_mouse_pos = use_signal(|| (0, 0));
    let input_ports = node.input_ports();
    let output_ports = node.output_ports();
    let port_height_factor = usize_to_f64(output_ports.len().max(input_ports.len()));
    let node_size = NodesStore::size();

    // let is_active = if node.is_active() { "active-node" } else { "" };
    let _z_index = node.z_index();
    let header_scale = 0.3;
    let id = *node.id();
    let z_index = node.z_index();
    //let is_active = node.is_active();
    rsx! {
        div {
            class: "node",
            style: format!(
                "transform-origin: center; position: absolute; left: {}px; top: {}px; z-index: {z_index};",
                shift().0,
                shift().1,
            ),
            onmousedown: move |event: MouseEvent| {
                println!("Node mouse down");
                current_mouse_pos
                    .set((
                        event.client_coordinates().x as i32,
                        event.client_coordinates().y as i32,
                    ));
                editor_status.drag_status.set(DragStatus::Node(id));
                node_store.set_node_active(id, z_index);
                node_activated.set(Some(id));
                event.stop_propagation();
            },
            onmouseup: move |event: MouseEvent| {
                println!("Node mouse up");
                editor_status.drag_status.set(DragStatus::None);
                event.stop_propagation();
            },
            onmousemove: move |event| {
                let drag_status = &*(editor_status.drag_status.read());
                println!("Node drag_status: {:?}", drag_status);
                match drag_status {
                    DragStatus::Node(_id) => {
                        let rel_shift_x = event.client_coordinates().x as i32
                            - current_mouse_pos().0;
                        let rel_shift_y = event.client_coordinates().y as i32
                            - current_mouse_pos().1;
                        current_mouse_pos
                            .set((
                                event.client_coordinates().x as i32,
                                event.client_coordinates().y as i32,
                            ));
                        shift.set((shift().0 + rel_shift_x, shift().1 + rel_shift_y));
                        event.stop_propagation();
                    }
                    DragStatus::Graph => {}
                    _ => {}
                }
            },
            onclick: move |_| {
                println!("Node clicked");
                node_activated.set(Some(node.id));
            },
            oncontextmenu: use_node_context_menu(*node.id()),

            GraphNodeContent {
                node_header: rsx! {
                    GraphNodeHeader { node_name: node.name(), node_id: *node.id(), node_size }
                },
                node_body: rsx! {
                    div {
                        class: "node-body",
                        style: format!(
                            "height: {}px;",
                            node_size.y.mul_add(1. - header_scale, (port_height_factor - 1.) * 32.),
                        ),
                        NodePorts {
                            node_width: node_size.x,
                            node_height: node_size.y * (1. - header_scale),
                            node_id: *node.id(),
                            input_ports: input_ports.clone(),
                            output_ports: output_ports.clone(),
                        }
                    }
                },
                node_size,
            }
        }
    }
}
#[must_use]
fn use_node_context_menu(node_id: Uuid) -> Callback<Event<MouseData>> {
    use_callback(move |evt: Event<MouseData>| {
        println!("Node context menu clicked");
        evt.prevent_default();
        let mut cx_menu = CONTEXT_MENU.write();
        *cx_menu = CxMenu::new(
            evt.page_coordinates().x,
            evt.page_coordinates().y,
            vec![("Delete node".to_owned(), use_delete_node(node_id))],
        );
    })
}
#[must_use]
fn use_delete_node(node_id: Uuid) -> Callback<Event<MouseData>> {
    use_callback(move |_: Event<MouseData>| {
        let node_id = node_id;
        spawn(async move {
            match api::delete_node(&HTTP_API_CLIENT(), node_id).await {
                Ok(_id_vec) => {
                    // for id in &id_vec {
                    //    node_store.delete_node(*id);
                    // }
                }
                Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
            }
        });
    })
}
