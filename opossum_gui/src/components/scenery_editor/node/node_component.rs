#![allow(clippy::derive_partial_eq_without_eq)]
use super::NodeElement;
use crate::{
    api::{self},
    components::{
        context_menu::cx_menu::CxMenu,
        graph_node::graph_node_components::{GraphNodeContent, GraphNodeHeader},
        scenery_editor::{nodes::NodesStore, ports::ports_component::NodePorts},
    },
    CONTEXT_MENU, HTTP_API_CLIENT, NODES_STORE, OPOSSUM_UI_LOGS, ZOOM,
};
use dioxus::prelude::*;
use opossum_backend::usize_to_f64;
use uuid::Uuid;

#[component]
pub fn Node(node: NodeElement, node_activated: Signal<bool>) -> Element {
    let zoom = ZOOM.read().current();

    let input_ports = node.input_ports();
    let output_ports = node.output_ports();
    let port_height_factor = usize_to_f64(output_ports.len().max(input_ports.len()));
    let on_mouse_down = {
        let id = *node.id();
        let z_index = node.z_index();
        let is_active = node.is_active();
        move |event: MouseEvent| {
            event.prevent_default();
            if !is_active {
                NODES_STORE.write().set_node_active(id, z_index);
            }
        }
    };
    let node_size = NodesStore::size();
    let (x, y) = (node.x() - node_size.x / 2., node.y() - node_size.y / 2.);

    let is_active = if node.is_active() { "active-node" } else { "" };
    let z_index = node.z_index();
    let header_scale = 0.3;

    rsx! {
        div {
            draggable: "true",
            class: "node draggable prevent-select {is_active}",
            style: "transform: scale({zoom}); transform-origin: center; position: absolute; left: {x}px; top: {y}px; z-index: {z_index};",
            onmousedown: on_mouse_down,
            onclick: move |_| {
                println!("Node clicked");
                if !node.is_active {
                    node_activated.set(true);
                }
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
pub fn use_node_context_menu(node_id: Uuid) -> Callback<Event<MouseData>> {
    use_callback(move |evt: Event<MouseData>| {
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
pub fn use_delete_node(node_id: Uuid) -> Callback<Event<MouseData>> {
    use_callback(move |_: Event<MouseData>| {
        let node_id = node_id;
        spawn(async move {
            match api::delete_node(&HTTP_API_CLIENT(), node_id).await {
                Ok(id_vec) => {
                    for id in &id_vec {
                        NODES_STORE.write().delete_node(*id);
                    }
                }
                Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
            }
        });
    })
}
