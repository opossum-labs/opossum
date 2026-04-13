#![allow(clippy::derive_partial_eq_without_eq)]
use std::collections::HashSet;

use super::NodeElement;
use crate::CONTEXT_MENU;
use crate::components::scenery_editor::constants::HEADER_HEIGHT;
use crate::components::scenery_editor::graph_workspace::{GraphStateStoreExt, GraphStoreStoreExt};
use crate::components::scenery_editor::{DragStatus, GraphStore};
use crate::components::scenery_editor::{GraphState, GraphsWorkspaceState};
use crate::components::{
    context_menu::cx_menu::{CxMenu, CxtCommand},
    scenery_editor::{
        constants::{BORDER_WIDTH, NODE_WIDTH},
        node::graph_node_components::GraphNodeContent,
        ports::ports_component::NodePorts,
        {GraphsWorkspaceAction, NodeType},
    },
};
use dioxus::html::geometry::euclid::default::{Point2D, Rect, Size2D};
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use opossum_core::types::api_types::NewRefNode;
use uuid::Uuid;

#[component]
pub fn Node(
    node: ReadStore<NodeElement>,
    ctrl_pressed: ReadSignal<bool>,
    shift_pressed: ReadSignal<bool>,
    mouse_pos_in_editor: Memo<Point2D<f64>>,
    nodes_in_selection: Memo<HashSet<Uuid>>,
) -> Element {
    let node = node();
    let graph_state = use_context::<ReadStore<GraphState>>();
    let graph_store = graph_state.graph_store();
    let graph_id = graph_state.graph_info().read().id;
    let workspace = use_context::<ReadSignal<GraphsWorkspaceState>>();
    let drag_status = workspace.peek().drag_status;
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let position = node.pos();
    let node_height = node.node_body_height() + HEADER_HEIGHT;
    let active_node_ids = graph_store().selected_node_ids();
    let active_optical_node_ids = graph_store().selected_optical_nodes();
    let node_id = node.id();
    let is_optical_node = node.is_optical_node();
    let is_active = if active_node_ids.contains(&node.id()) {
        "active-node"
    } else {
        ""
    };

    println!("node rerender: {}", node.id().as_simple());

    let in_selection_box_class = use_memo(move || {
        let in_selection = nodes_in_selection.read().contains(&node_id);
        let already_selected = graph_store
            .node_selection()
            .read()
            .all_nodes
            .read()
            .contains_key(&node_id);

        if in_selection {
            if ctrl_pressed() && already_selected {
                "node-selection-remove"
            } else {
                "node-selection"
            }
        } else {
            ""
        }
        .to_string()
    });

    let z_index = node.z_index();

    let node_icon = node.node_type.icon();
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
                    if Some(MouseButton::Primary) == event.trigger_button() {
                        workspace_processor
                            .send(GraphsWorkspaceAction::SetDragStatus(DragStatus::NodeInit));
                        workspace_processor
                            .send(GraphsWorkspaceAction::NodeClick {
                                graph_id,
                                node_id,
                                is_optical_node,
                                z_index,
                                ctrl_pressed: ctrl_pressed(),
                            });
                    }
                    event.stop_propagation();
                }
            },
            onmousemove: move |_| {
                let drag_status = workspace.read().drag_status.read().clone();
                if drag_status == DragStatus::NodeInit {
                    workspace_processor
                        .send(GraphsWorkspaceAction::SetDragStatus(DragStatus::Nodes));
                }
            },
            onmouseup: {
                move |_| {
                    let drag_status = workspace.read().drag_status.read().clone();

                    if drag_status == DragStatus::NodeInit && !ctrl_pressed() {
                        workspace_processor
                            .send(GraphsWorkspaceAction::SetNodeActive {
                                graph_id,
                                node_id,
                                is_optical_node,
                                z_index,
                            });
                    }
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
                        let mut cx_menu = CxMenu::new(
                            event.page_coordinates().x,
                            event.page_coordinates().y,
                            vec![
                                (
                                    "Create reference".to_owned(),
                                    CxtCommand::AddRefNode(new_ref_node),
                                ),
                            ],
                        );

                        if active_optical_node_ids.len() > 1 {
                            cx_menu
                                .entries
                                .push((
                                    "Group optical nodes".to_owned(),
                                    CxtCommand::ConvertToGroup {
                                        nodes: active_optical_node_ids.iter().copied().collect(),
                                        graph_id,
                                    },
                                ));
                        }
                        let mut ctx = CONTEXT_MENU.write();
                        *ctx = Some(cx_menu);
                    }
                }
            },
            ondoubleclick: {
                let node = node;
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
