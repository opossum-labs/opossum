#![allow(clippy::derive_partial_eq_without_eq)]
use super::NodeElement;
use crate::CONTEXT_MENU;
use crate::components::{
    context_menu::cx_menu::{CxMenu, CxtCommand},
    scenery_editor::{
        constants::{BORDER_WIDTH, NODE_WIDTH},
        graph_editor::graph_editor_component::{DragStatus, EditorState},
        graph_store::GraphStore,
        node::graph_node_components::GraphNodeContent,
        ports::ports_component::NodePorts,
        {GraphState, GraphsWorkspaceAction, NodeType},
    },
};
use dioxus::prelude::*;
use opossum_core::types::api_types::NewRefNode;
use uuid::Uuid;

#[component]
pub fn Node(node: NodeElement, add_new_group_tab_handler: EventHandler<(String, Uuid)>) -> Element {
    let mut editor_status = use_context::<Signal<EditorState>>();
    let graph_store = use_context::<Signal<GraphStore>>();
    let graph_state = use_context::<Signal<GraphState>>();
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let position = node.pos();
    let active_node_id = graph_store().active_node();
    let is_active = active_node_id.map_or("", |active_node_id| {
        if active_node_id == node.id() {
            "active-node"
        } else {
            ""
        }
    });
    let node_id = node.id();
    let z_index = node.z_index();
    let node_icon = node.node_type.icon();
    let is_optical_node = node.is_optical_node();
    rsx! {
        div {
            id: format!("node_container_{}", node_id.as_simple()),
            tabindex: 0, // necessary to allow to receive keyboard focus
            class: "node {is_active}",
            draggable: false,
            style: format!(
                "left: {}px; top: {}px; transform: translate({}px, {}px); z-index: {z_index}; border-width:{}px",
                position.x.trunc(),
                position.y.trunc(),
                position.x.fract(),
                position.y.fract(),
                BORDER_WIDTH,
            ),
            onmousedown: {
                let z_index = node.z_index();
                move |event: MouseEvent| {
                    editor_status.write().drag_status.set(DragStatus::Node(node_id, position));
                    let previously_selected = graph_store().active_node();
                    if previously_selected != Some(node_id) {
                        graph_store().set_node_active(node_id, z_index);
                    }
                    event.stop_propagation();
                }
            },
            onkeydown: move |event| {
                if event.data().key() == Key::Delete {
                    if !is_active.is_empty() {
                        workspace_processor
                            .send(GraphsWorkspaceAction::DeleteNode {
                                node_id,
                                graph_id: graph_state.read().id,
                            });
                    }
                    event.stop_propagation();
                }
            },
            oncontextmenu: {
                move |event: Event<MouseData>| {
                    event.prevent_default();
                    if is_optical_node {
                        let new_ref_node = NewRefNode::new(
                            node_id,
                            (position.x + NODE_WIDTH, position.y + 100.0),
                        );
                        let cx_menu = CxMenu::new(
                            event.page_coordinates().x,
                            event.page_coordinates().y,
                            vec![
                                (
                                    "Create reference".to_owned(),
                                    CxtCommand::AddRefNode(new_ref_node),
                                ),
                            ],
                        );
                        let mut ctx = CONTEXT_MENU.write();
                        *ctx = Some(cx_menu);
                    }
                }
            },
            ondoubleclick: {
                let node = node.clone();
                move |_| {
                    if let NodeType::Optical(node_type) = node.node_type()
                        && node_type == "group"
                    {
                        add_new_group_tab_handler.call((node.name(), node.id()));
                    }
                }
            },
            GraphNodeContent {
                name: node.name(),
                node_type: node.node_type().clone(),
                body: rsx! {
                    div {
                        class: "node-body",
                        draggable: false,
                        style: format!("height: {}px;", node.node_body_height()),
                        if node_icon.is_some() {
                            img { src: node_icon.unwrap(), draggable: false }
                        }
                        NodePorts { node: node.clone(), inverted: node.inverted() }
                    }
                },
            }
        }
    }
}
