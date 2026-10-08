// File: opossum_gui/src/components/scenery_editor/node/node_component.rs

use super::{AMP_STATUS_HEIGHT, NodeElement};
use crate::{
    CONTEXT_MENU,
    components::{
        context_menu::cx_menu::{CxMenu, CxtCommand},
        scenery_editor::{
            DragStatus, GraphState, GraphsWorkspaceAction, GraphsWorkspaceState,
            GraphsWorkspaceStateStoreExt, NodeType,
            constants::{BORDER_WIDTH, NODE_WIDTH},
            graph_workspace::GraphStateStoreExt,
            node::{graph_node_components::GraphNodeContent, node_icon::NodeSymbolIcon},
            ports::ports_component::NodePorts,
        },
    },
};
use dioxus::{
    html::{geometry::euclid::default::Point2D, input_data::MouseButton},
    prelude::*,
};
use opossum_core::{nodes::is_volume_node_type, types::api_types::NewRefNode};
use std::collections::HashSet;
use uuid::Uuid;

#[component]
pub fn Node(
    node: NodeElement,
    graph_id: Uuid,
    is_active: bool,
    is_drop_group: bool,
    ctrl_pressed: ReadSignal<bool>,
    nodes_in_selection: Memo<HashSet<Uuid>>,
) -> Element {
    let graph_state = use_context::<ReadStore<GraphState>>();
    let graph_store = graph_state.graph_store();
    let workspace = use_context::<ReadStore<GraphsWorkspaceState>>();
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();

    let position = node.pos();
    let node_id = node.id();
    let is_optical_node = node.is_optical_node();

    let is_volume_node = matches!(
        node.node_type(),
        NodeType::Optical(node_type) if is_volume_node_type(node_type)
    );
    let is_amplifier = node.is_amplifier_candidate();

    let active_class = if is_active { "active-node" } else { "" };
    let drop_group_class = if is_drop_group { "drop-group" } else { "" };

    let in_selection_box_class = use_memo(move || {
        let in_selection = nodes_in_selection.read().contains(&node_id);
        if in_selection {
            if ctrl_pressed() && is_active {
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
    let test_id = format!("node-{}", node.node_index());
    let symbol_id = node.node_type().symbol_id();

    rsx! {
        div {
            "data-testid": "{test_id}",
            id: format!("node_container_{}", node_id.as_simple()),
            tabindex: 0,
            class: "node {active_class} {in_selection_box_class} {drop_group_class}",
            draggable: false,
            style: format!(
                "left: {}px; top: {}px; transform: translate({}px, {}px); z-index: {z_index}; border-width:{}px",
                position.x.trunc(),
                position.y.trunc(),
                position.x.fract(),
                position.y.fract(),
                BORDER_WIDTH,
            ),
            onmousedown: move |event: MouseEvent| {
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
            },
            onmouseup: {
                let z_index = node.z_index();
                move |_| {
                    // Peek the drag status without creating a reactive subscription in Dioxus
                    let was_plain_click = *workspace.drag_status().peek()
                        == DragStatus::NodeInit;
                    if was_plain_click && !ctrl_pressed() {
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
            oncontextmenu: move |event: Event<MouseData>| {
                event.prevent_default();
                if is_optical_node {
                    let new_ref_node = NewRefNode::new(
                        node_id,
                        (position.x + NODE_WIDTH, position.y + 100.0),
                    );
                    let mut cx_menu = CxMenu::new(
                        event.page_coordinates().x,
                        event.page_coordinates().y,
                        vec![],
                    );

                    // Query selected optical nodes on demand only
                    let active_optical_node_ids = graph_store.peek().selected_optical_nodes();
                    if active_optical_node_ids.len() <= 1 {
                        cx_menu
                            .add_entry((
                                "Create reference".to_owned(),
                                CxtCommand::AddRefNode(new_ref_node),
                            ));
                    } else {
                        cx_menu
                            .add_entry((
                                "Group optical nodes".to_owned(),
                                CxtCommand::ConvertToGroup {
                                    nodes: active_optical_node_ids.into_iter().collect(),
                                    graph_id,
                                },
                            ));
                    }
                    if is_volume_node {
                        let (label, target_state) = if is_amplifier {
                            ("As passive optic", false)
                        } else {
                            ("As amplifier", true)
                        };
                        cx_menu
                            .add_entry((
                                label.to_owned(),
                                CxtCommand::ToggleAmplifierCandidate {
                                    node_id,
                                    graph_id,
                                    is_amplifier: target_state,
                                },
                            ));
                    }
                    let mut ctx = CONTEXT_MENU.write();
                    *ctx = Some(cx_menu);
                }
            },
            ondoubleclick: move |_| {
                if let NodeType::Optical(node_type) = node.node_type()
                    && node_type == "group"
                {
                    workspace_processor
                        .send(GraphsWorkspaceAction::OpenGroupTab {
                            group_id: node.id(),
                            group_name: node.name(),
                        });
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
                        NodeSymbolIcon { symbol_id }
                        NodePorts { node: node.clone(), inverted: node.inverted() }
                    }
                },
                footer: node.is_amplifier_candidate()
                    .then(|| {
                        let amp_model = node.amp_model().unwrap_or("None");
                        rsx! {
                            div {
                                class: "node-amp-status",
                                pointer_events: "none",
                                style: format!("height: {AMP_STATUS_HEIGHT}px;"),
                                "amp: {amp_model}"
                            }
                        }
                    }),
            }
        }
    }
}
