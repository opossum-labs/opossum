#![allow(clippy::derive_partial_eq_without_eq)]
use super::NodeElement;
use crate::CONTEXT_MENU;
use crate::components::scenery_editor::GraphsWorkspaceState;
use crate::components::scenery_editor::constants::HEADER_HEIGHT;
use crate::components::scenery_editor::graph_editor::{DragStatus, GraphState, GraphStore};
use crate::components::{
    context_menu::cx_menu::{CxMenu, CxtCommand},
    scenery_editor::{
        constants::{BORDER_WIDTH, NODE_WIDTH},
        node::graph_node_components::GraphNodeContent,
        ports::ports_component::NodePorts,
        {GraphsWorkspaceAction, NodeType},
    },
};
use dioxus::html::geometry::euclid::default::{Rect, Size2D};
use dioxus::prelude::*;
use opossum_core::types::api_types::NewRefNode;

#[component]
pub fn Node(node: NodeElement, ctrl_pressed: Signal<bool>, shift_pressed: Signal<bool>) -> Element {
    let graph_store = use_context::<Signal<GraphStore>>();
    let graph_state = use_context::<Signal<GraphState>>();
    let mut workspace = use_context::<Signal<GraphsWorkspaceState>>();

    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let position = node.pos();
    let node_height = node.node_body_height() + HEADER_HEIGHT;
    let active_node_ids = graph_store().selected_nodes();
    let node_id = node.id();
    let is_active = if active_node_ids.contains(&node.id()) {
        "active-node"
    } else {
        ""
    };
    let in_selection_box_class = use_memo(move || {
        {
            let node_rect = Rect::new(position, Size2D::new(NODE_WIDTH, node_height));
            if let Some(select_box) = *workspace.read().selection_box.read()
                && select_box.intersects(&node_rect)
            {
                graph_store().to_be_selected.write().insert(node_id);
                "node-selection"
            } else {
                graph_store().to_be_selected.write().remove(&node_id);
                ""
            }
        }
        .to_string()
    });
    let z_index = node.z_index();
    let node_icon = node.node_type.icon();
    let is_optical_node = node.is_optical_node();
    rsx! {
        div {
            id: format!("node_container_{}", node_id.as_simple()),
            tabindex: 0, // necessary to allow to receive keyboard focus
            class: "node {is_active} {in_selection_box_class}",
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
                    workspace.write().drag_status.set(DragStatus::Node(node_id, position));
                    if ctrl_pressed() {
                        if graph_store().selected_nodes().contains(&node_id) {
                            graph_store().remove_from_node_selection(node_id);
                        }
                        else{
                            graph_store().add_to_node_selection(node_id);
                        }
                    }
                    else if shift_pressed(){
                    }
                    else{
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
                                graph_id: graph_state.read().graph_info.id,
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
                        workspace_processor
                            .send(GraphsWorkspaceAction::OpenGroupTab {
                                group_id: node.id(),
                                group_name: node.name(),
                            });
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
