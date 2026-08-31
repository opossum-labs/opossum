use super::{AMP_STATUS_HEIGHT, NodeElement};
use crate::{
    CONTEXT_MENU,
    components::{
        context_menu::cx_menu::{CxMenu, CxtCommand},
        scenery_editor::{
            DragStatus, GraphState, GraphsWorkspaceAction, GraphsWorkspaceState, NodeType,
            constants::{BORDER_WIDTH, NODE_WIDTH},
            graph_workspace::{
                GraphStateStoreExt, GraphStoreStoreExt, GraphsWorkspaceStateStoreExt,
            },
            node::graph_node_components::GraphNodeContent,
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
    let workspace = use_context::<ReadStore<GraphsWorkspaceState>>();
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let position = node.pos();
    let active_node_ids = graph_store().selected_node_ids();
    let active_optical_node_ids = graph_store().selected_optical_nodes();
    let node_id = node.id();
    let is_optical_node = node.is_optical_node();
    // Only components with a physical volume can amplify - they are the ones a `Volumetric` node
    // registers as. The canvas knows a node by its type name, so it asks the core rather than
    // keeping a list of its own. Whether a node *is* an amplifier is a hardware fact independent of
    // any pump scenario, so this no longer needs one to be active to offer the entry.
    let is_volume_node = matches!(
        node.node_type(),
        NodeType::Optical(node_type) if is_volume_node_type(node_type)
    );
    // The amp entry is a toggle, so it needs the node's current candidacy. That state is already on
    // the canvas (see `NodeElement::is_amplifier_candidate`), so a right-click costs no request.
    let is_amplifier = node.is_amplifier_candidate();
    let is_active = if active_node_ids.contains(&node.id()) {
        "active-node"
    } else {
        ""
    };

    let is_drop_group = if let Some((id, _)) = *workspace.drop_in_group().read()
        && id == node_id
    {
        "drop-group"
    } else {
        ""
    };

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

    // Construct the deterministic test ID for Playwright (e.g., "node-1", "node-2")
    let test_id = format!("node-{}", node.node_index());

    let node_icon = node.node_type.icon();
    rsx! {
        div {
            // Assign deterministic test ID attribute for E2E testing
            "data-testid": "{test_id}",
            id: format!("node_container_{}", node_id.as_simple()),
            tabindex: 0, // necessary to allow to receive keyboard focus
            class: "node {is_active} {in_selection_box_class} {is_drop_group}",
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
                if *workspace.drag_status().read() == DragStatus::NodeInit {
                    workspace_processor
                        .send(GraphsWorkspaceAction::SetDragStatus(DragStatus::Nodes));
                }
            },
            onmouseup: {
                move |_| {
                    if *workspace.drag_status().read() == DragStatus::NodeInit && !ctrl_pressed()
                    {
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

                        // Initialize the context menu with an empty entries vector
                        let mut cx_menu = CxMenu::new(
                            event.page_coordinates().x,
                            event.page_coordinates().y,
                            vec![],
                        );

                        // Conditional rendering of menu items based on the selection count
                        if active_optical_node_ids.len() <= 1 {
                            // Show reference option only if 0 or 1 optical nodes are selected
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
                                        nodes: active_optical_node_ids.iter().copied().collect(),
                                        graph_id,
                                    },
                                ));
                        }
                        if is_volume_node {
                            // Offer the way back, too: an accidentally marked node must be curable
                            // from the same menu it was marked an amplifier candidate in.
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
                }
            },
            ondoubleclick: {
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
                        if let Some(icon_url) = node_icon {
                            img { src: icon_url, draggable: false }
                        }
                        NodePorts { node: node.clone(), inverted: node.inverted() }
                    }
                },
                // Shown for every amplifier candidate, not only while it actively amplifies in the
                // current scenario - candidacy has to stay visible (as "None") so it's obvious this
                // node is a potential amplifier while editing its other properties, even with no
                // scenario active or with this scenario leaving it passive.
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
