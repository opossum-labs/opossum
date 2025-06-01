use dioxus::prelude::*;
use opossum::optic_ports::PortType;
use crate::components::scenery_editor::{
    edges::edges_component::{EdgePort, NewEdgeCreationStart},
    graph_editor::graph_editor_component::{DragStatus, EditorState},
    node::{NodeElement, PORT_HEIGHT, PORT_WIDTH},
};
#[derive(Clone, Eq, PartialEq, Default)]
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
    #[must_use]
    pub const fn input_ports(&self) -> &Vec<String> {
        &self.input_ports
    }
    #[must_use]
    pub const fn output_ports(&self) -> &Vec<String> {
        &self.output_ports
    }
}

#[component]
pub fn NodePort(node: NodeElement, port_name: String, port_type: PortType) -> Element {
    let mut editor_status = use_context::<EditorState>();
    let rel_port_position = node.rel_port_position(&port_type, &port_name);
    let abs_port_position = node.abs_port_position(&port_type, &port_name);
    let node_id = node.id();
    let port_class = if port_type == PortType::Input {
        "input-port"
    } else {
        "output-port"
    };
    rsx! {
        div {
            class: "port {port_class}",
            style: format!(
                "left: {}px; top: {}px; width: {}px; height: {}px;",
                rel_port_position.x,
                rel_port_position.y,
                PORT_WIDTH,
                PORT_HEIGHT,
            ),
            draggable: false,
            onmousedown: {
                let port_name = port_name.clone();
                let port_type = port_type.clone();
                move |event: MouseEvent| {
                    editor_status
                        .drag_status
                        .set(
                            DragStatus::Edge(NewEdgeCreationStart {
                                src_node: node_id,
                                src_port: port_name.clone(),
                                src_port_type: port_type.clone(),
                                start_pos: abs_port_position,
                            }),
                        );
                    event.stop_propagation();
                }
            },
            onmouseenter: {
                move |event: MouseEvent| {
                    let edge_increation = editor_status.edge_in_creation.read().clone();
                    if let Some(mut edge_in_creation) = edge_increation {
                        edge_in_creation
                            .set_end_port(
                                Some(EdgePort {
                                    node_id,
                                    port_name: port_name.clone(),
                                    port_type: port_type.clone(),
                                }),
                            );
                        editor_status.edge_in_creation.set(Some(edge_in_creation));
                        event.stop_propagation();
                    }
                }
            },
            onmouseleave: {
                move |event: MouseEvent| {
                    let edge_increation = editor_status.edge_in_creation.read().clone();
                    if let Some(mut edge_in_creation) = edge_increation {
                        edge_in_creation.set_end_port(None);
                        editor_status.edge_in_creation.set(Some(edge_in_creation));
                        event.stop_propagation();
                    }
                }
            },
        }
    }
}

#[component]
pub fn NodePorts(node: NodeElement) -> Element {
    rsx! {
        for in_port in node.input_ports() {
            NodePort {
                node: node.clone(),
                port_name: in_port,
                port_type: PortType::Input,
            }
        }
        for out_port in node.output_ports() {
            NodePort {
                node: node.clone(),
                port_name: out_port,
                port_type: PortType::Output,
            }
        }
    }
}
