#![allow(clippy::derive_partial_eq_without_eq)]
use super::NodeElement;
use crate::CONTEXT_MENU;
use crate::components::scenery_editor::constants::HEADER_HEIGHT;
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

#[component]
pub fn Node(
    node: NodeElement,
    ctrl_pressed: Signal<bool>,
    shift_pressed: Signal<bool>,
    mouse_pos_in_editor: Memo<Point2D<f64>>,
) -> Element {
    let graph_store = use_context::<ReadSignal<GraphStore>>();
    let graph_state = use_context::<ReadSignal<GraphState>>();
    let graph_id = graph_state.read().graph_info.id;
    let workspace = use_context::<ReadSignal<GraphsWorkspaceState>>();
    let drag_status = workspace.read().drag_status;
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
    let in_selection_box_class = use_memo(move || {
        {
            let node_rect = Rect::new(position, Size2D::new(NODE_WIDTH, node_height));
            let selection_box = *workspace.peek().selection_box.read();
            if let Some(select_box) = selection_box
                && select_box.intersects(&node_rect)
            {
                let is_contained = graph_store
                    .peek()
                    .node_selection
                    .peek()
                    .all_nodes
                    .peek()
                    .contains_key(&node_id);
                if ctrl_pressed() && is_contained {
                    workspace_processor.send(GraphsWorkspaceAction::AddToToBeRemoved {
                        graph_id,
                        node_id,
                        is_optical_node,
                    });
                    "node-selection-remove"
                } else {
                    workspace_processor.send(GraphsWorkspaceAction::AddToToBeSelected {
                        graph_id,
                        node_id,
                        is_optical_node,
                    });
                    "node-selection"
                }
            } else {
                workspace_processor
                    .send(GraphsWorkspaceAction::RemoveFromToBeSelected { graph_id, node_id });
                ""
            }
        }
        .to_string()
    });

    let node_type = node.node_type().clone();
    let z_index = node.z_index();
    use_effect({
        move || {
            let mouse_pos = *mouse_pos_in_editor.read();
            let mut droppable_group = *workspace.peek().drop_in_group.read();
            let selected_nodes = graph_store
                .peek()
                .node_selection
                .read()
                .all_nodes
                .read()
                .clone();

            if !selected_nodes.contains_key(&node_id)
                && let NodeType::Optical(node_type) = &node_type
                && node_type == "group"
                && *drag_status.peek() == DragStatus::Nodes
            {
                let node_rect = Rect::new(position, Size2D::new(NODE_WIDTH, node_height));
                let contains = node_rect.contains(mouse_pos);
                if contains {
                    if let Some((_, g_z_index)) = droppable_group
                        && z_index > g_z_index
                    {
                        droppable_group = Some((node_id, z_index));
                    } else if droppable_group.is_none() {
                        droppable_group = Some((node_id, z_index));
                    }
                } else if let Some((g_id, _)) = droppable_group
                    && g_id == node_id
                {
                    droppable_group = None;
                }
                if *workspace.peek().drop_in_group.read() != droppable_group {
                    workspace_processor
                        .send(GraphsWorkspaceAction::SetDropInGroup(droppable_group));
                }
            }
        }
    });
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
