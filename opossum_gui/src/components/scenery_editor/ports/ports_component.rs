use crate::{
    CONTEXT_MENU,
    components::{
        context_menu::cx_menu::{CxMenu, CxtCommand},
        scenery_editor::{
            EditorState, GraphStore, GraphsWorkspaceState,
            constants::{BORDER_WIDTH, PORT_HEIGHT, PORT_MAP_DIST, PORT_WIDTH},
            edges::edges_component::{EdgePort, NewEdgeCreationStart},
            graph_editor::DragStatus,
            node::NodeElement,
        },
    },
};
use dioxus::{html::input_data::MouseButton, prelude::*};
use opossum_core::prelude::*;
use uuid::Uuid;
#[derive(Clone, Eq, PartialEq, Default, Debug)]
pub struct Ports {
    input_ports: Vec<String>,
    output_ports: Vec<String>,
}
impl Ports {
    #[must_use]
    pub const fn new(input_ports: Vec<String>, output_ports: Vec<String>) -> Self {
        Self {
            input_ports,
            output_ports,
        }
    }
    pub fn set_input_ports(&mut self, ports: Vec<String>) {
        self.input_ports = ports;
    }
    pub fn set_output_ports(&mut self, ports: Vec<String>) {
        self.output_ports = ports;
    }
    pub fn remove_input_port(&mut self, remove: &String) {
        self.input_ports.retain(|p| p != remove);
    }
    pub fn remove_output_port(&mut self, remove: &String) {
        self.output_ports.retain(|p| p != remove);
    }
    #[must_use]
    pub const fn input_ports(&self) -> &Vec<String> {
        &self.input_ports
    }
    #[must_use]
    pub const fn output_ports(&self) -> &Vec<String> {
        &self.output_ports
    }
    pub fn invert_ports(&mut self) {
        let input_buffer = self.input_ports.clone();
        self.input_ports = self.output_ports.clone();
        self.output_ports = input_buffer;
    }
}

#[component]
pub fn NodePort(
    node: NodeElement,
    port_name: String,
    port_type: PortType,
    inverted_node: bool,
) -> Element {
    let mut editor_status = use_context::<Signal<EditorState>>();
    let graph_store = use_context::<Signal<GraphStore>>();
    let mut workspace = use_context::<Signal<GraphsWorkspaceState>>();

    let rel_port_position = node.rel_port_position(&port_type, &port_name);
    let abs_port_position = node.abs_port_position(&port_type, &port_name);

    let node_id = node.id();
    let mut port_class = if inverted_node {
        if port_type == PortType::Input {
            "output-port"
        } else {
            "input-port"
        }
    } else if port_type == PortType::Input {
        "input-port"
    } else {
        "output-port"
    }
    .to_string();

    let is_mapped_input = port_type == PortType::Input && graph_store
        .read()
        .mapped_ports
        .read()
        .contains_port_of_node(node.id(), &port_name);
    let is_mapped_output = port_type == PortType::Output && graph_store
        .read()
        .mapped_ports
        .read()
        .contains_port_of_node(node.id(), &port_name);

    rsx! {
        div {
            class: "port {port_class}",
            title: "{port_name}",
            style: format!(
                "left: {}px; top: {}px; width: {}px; height: {}px; border-width: {}px; transform: translateX(-50%) translateX({}px) translateY(-50%)",
                rel_port_position.x,
                rel_port_position.y,
                PORT_WIDTH,
                PORT_HEIGHT,
                BORDER_WIDTH,
                -BORDER_WIDTH / 2.,
            ),
            draggable: false,

            onmousedown: {
                let port_name = port_name.clone();
                move |event: MouseEvent| {
                    if Some(MouseButton::Primary) == event.trigger_button() {
                        workspace
                            .write()
                            .drag_status
                            .set(
                                DragStatus::Edge(NewEdgeCreationStart {
                                    src_node: node_id,
                                    src_port: port_name.clone(),
                                    src_port_type: port_type,
                                    start_pos: abs_port_position,
                                }),
                            );
                        event.stop_propagation();
                    }
                }
            },
            onmouseenter: {
                let port_name = port_name.clone();
                move |event: MouseEvent| {
                    let edge_increation = editor_status.read().edge_in_creation.read().clone();
                    if let Some(mut edge_in_creation) = edge_increation
                    // && !is_mapped_input
                    //     && !is_mapped_output
                    {
                        edge_in_creation
                            .set_end_port(
                                Some(EdgePort {
                                    node_id,
                                    port_name: port_name.clone(),
                                    port_type,
                                }),
                            );
                        editor_status.write().edge_in_creation.set(Some(edge_in_creation));
                        event.stop_propagation();
                    }
                }
            },
            onmouseleave: {
                move |event: MouseEvent| {
                    let edge_increation = editor_status.read().edge_in_creation.read().clone();
                    event.stop_propagation();
                    if let Some(mut edge_in_creation) = edge_increation {
                        edge_in_creation.set_end_port(None);
                        editor_status.write().edge_in_creation.set(Some(edge_in_creation));
                    }
                }
            },
            oncontextmenu: {
                let port_name = port_name.clone();
                move |event: Event<MouseData>| {
                    event.prevent_default();
                    event.stop_propagation();
                    let active_tab = *workspace.read().active_tab.read();
                    let root_tab = *workspace.read().root_scenery_id.read();
                    let mapped_external_port_opt = graph_store
                        .read()
                        .mapped_ports
                        .read()
                        .external_port_of_mapped_port(node_id, &port_name);

                    if active_tab != root_tab {
                        let cx_menu = if let Some(group_port_name) = mapped_external_port_opt {
                            CxMenu::new(
                                event.page_coordinates().x,
                                event.page_coordinates().y,
                                vec![
                                    (
                                        "Remove port map from group".to_owned(),
                                        CxtCommand::RemovePortMap {
                                            group_id: active_tab,
                                            group_port_name,
                                            port_type,
                                        },
                                    ),
                                ],
                            )
                        } else {
                            CxMenu::new(
                                event.page_coordinates().x,
                                event.page_coordinates().y,
                                vec![
                                    (
                                        "Map port to group".to_owned(),
                                        CxtCommand::MapNodePort {
                                            port_type,
                                            group_port_name: Uuid::new_v4().as_simple().to_string(),
                                            mapped_node_port_name: port_name.clone(),
                                            mapped_node_id: node.id(),
                                            group_id: active_tab,
                                        },
                                    ),
                                ],
                            )
                        };
                        let mut ctx = CONTEXT_MENU.write();
                        *ctx = Some(cx_menu);
                    }
                }
            },
        }

        if is_mapped_input {
            div {
                class: "port-map-wrapper",
                style: format!(
                    "left: {}px; top: {}px; transform: translate(-50%, -50%)",
                    rel_port_position.x - 2. * PORT_WIDTH - PORT_MAP_DIST - BORDER_WIDTH,
                    rel_port_position.y,
                ),
                oncontextmenu: {
                    let port_name = port_name.clone();
                    move |event: Event<MouseData>| {
                        event.prevent_default();
                        event.stop_propagation();

                        let active_tab = *workspace.read().active_tab.read();
                        let root_tab = *workspace.read().root_scenery_id.read();
                        if active_tab != root_tab {
                            let mapped_external_port_opt = graph_store
                                .read()
                                .mapped_ports
                                .read()
                                .external_port_of_mapped_port(node_id, &port_name);
                            if let Some(group_port_name) = mapped_external_port_opt {
                                let cx_menu = CxMenu::new(
                                    event.page_coordinates().x,
                                    event.page_coordinates().y,
                                    vec![
                                        (
                                            "Remove port map from group".to_owned(),
                                            CxtCommand::RemovePortMap {
                                                group_id: active_tab,
                                                group_port_name,
                                                port_type,
                                            },
                                        ),
                                    ],
                                );
                                let mut ctx = CONTEXT_MENU.write();
                                *ctx = Some(cx_menu);
                            }
                        }
                    }
                },

                div { class: "graph-port-node-input" }

                div {
                    class: "port-map-line",
                    style: format!("right: {}px; width: {}px;", -1.5 * PORT_WIDTH, PORT_MAP_DIST),
                }
            }
        }
        if is_mapped_output {
            div {
                class: "port-map-wrapper",
                style: format!(
                    "right: -{}px; top: {}px; transform: translate(-50%, -50%)",
                    rel_port_position.x - 2. * (PORT_WIDTH - BORDER_WIDTH) - PORT_MAP_DIST,
                    rel_port_position.y,
                ),
                oncontextmenu: {
                    let port_name = port_name.clone();
                    move |event: Event<MouseData>| {
                        event.prevent_default();
                        event.stop_propagation();

                        let active_tab = *workspace.read().active_tab.read();
                        let root_tab = *workspace.read().root_scenery_id.read();
                        if active_tab != root_tab {
                            let mapped_external_port_opt = graph_store
                                .read()
                                .mapped_ports
                                .read()
                                .external_port_of_mapped_port(node_id, &port_name);
                            if let Some(group_port_name) = mapped_external_port_opt {
                                let cx_menu = CxMenu::new(
                                    event.page_coordinates().x,
                                    event.page_coordinates().y,
                                    vec![
                                        (
                                            "Remove port map from group".to_owned(),
                                            CxtCommand::RemovePortMap {
                                                group_id: active_tab,
                                                group_port_name,
                                                port_type,
                                            },
                                        ),
                                    ],
                                );
                                let mut ctx = CONTEXT_MENU.write();
                                *ctx = Some(cx_menu);
                            }
                        }
                    }
                },

                div { class: "graph-port-node-output" }

                div {
                    class: "port-map-line",
                    style: format!("left: {}px; width: {}px;", -1.5 * PORT_WIDTH, PORT_MAP_DIST),
                }
            }
        }

    }
}

#[component]
pub fn NodePorts(node: NodeElement, inverted: bool) -> Element {
    let input_ports = node.input_ports();
    rsx! {
        for in_port in input_ports {
            NodePort {
                node: node.clone(),
                port_name: in_port,
                port_type: PortType::Input,
                inverted_node: inverted,
            }
        }
        for out_port in node.output_ports() {
            NodePort {
                node: node.clone(),
                port_name: out_port,
                port_type: PortType::Output,
                inverted_node: inverted,
            }
        }
    }
}
