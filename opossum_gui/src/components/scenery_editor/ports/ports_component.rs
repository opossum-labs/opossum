use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_backend::{usize_to_f64, PortType};
use uuid::Uuid;

use crate::{
    api::{self},
    components::scenery_editor::{
        edges::edges_component::{EdgeCreation, EdgeCreationPort, NewEdgeCreationStart},
        graph_editor::graph_editor_component::{DragStatus, EditorState},
        node::{NodeElement, PORT_HEIGHT, PORT_WIDTH},
        EDGES,
    },
    HTTP_API_CLIENT, OPOSSUM_UI_LOGS,
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
    // let nodes_store = use_context::<NodesStore>();
    // let optic_nodes = nodes_store.optic_nodes()();
    // let node_element = optic_nodes.get(&node_id).unwrap();
    // let node_pos= node_element.pos();
    // let node_pos = nodes_store.node_position(&node_id).unwrap();

    // let on_mouse_up = {
    //     let zoom_factor = 1.0;
    //     let port_name = port_name.clone();
    //     move |e: Event<MouseData>| {
    //         if let (Some(edge_start), Some(offset)) =
    //             (edge_in_creation.read().as_ref(), offset.read().offset())
    //         {
    //             let x = (-e.element_coordinates().x + port_w_h / 2.)
    //                 .mul_add(zoom_factor, e.page_coordinates().x - offset.0);
    //             let y = (-e.element_coordinates().y + port_w_h / 2.)
    //                 .mul_add(zoom_factor, e.page_coordinates().y - offset.1);

    //             let (
    //                 src_node,
    //                 src_port,
    //                 target_node,
    //                 target_port,
    //                 src_x,
    //                 src_y,
    //                 target_x,
    //                 target_y,
    //             ) = if edge_start.port_type() == &PortType::Output {
    //                 (
    //                     edge_start.node_id(),
    //                     edge_start.port_name(),
    //                     node_id,
    //                     port_name.clone(),
    //                     edge_start.start_x(),
    //                     edge_start.start_y(),
    //                     x,
    //                     y,
    //                 )
    //             } else {
    //                 (
    //                     node_id,
    //                     port_name.clone(),
    //                     edge_start.node_id(),
    //                     edge_start.port_name(),
    //                     x,
    //                     y,
    //                     edge_start.start_x(),
    //                     edge_start.start_y(),
    //                 )
    //             };

    //             spawn(async move {
    //                 let conn_info = ConnectInfo::new(
    //                     src_node,
    //                     src_port.clone(),
    //                     target_node,
    //                     target_port.clone(),
    //                     0.,
    //                 );
    //                 match api::post_add_connection(&HTTP_API_CLIENT(), conn_info).await {
    //                     Ok(conn_info) => EDGES.write().add_edge(Edge::new(
    //                         conn_info,
    //                         src_x,
    //                         src_y,
    //                         target_x,
    //                         target_y,
    //                         70. * zoom_factor,
    //                     )),
    //                     Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
    //                 }
    //             });
    //         }
    //         edge_in_creation.set(None);
    //     }
    // };
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
                let port_name = port_name.clone();
                let port_type = port_type.clone();
                move |event: MouseEvent| {
                    let edge_increation = editor_status.edge_in_creation.read().clone();
                    if let Some(mut edge_in_creation) = edge_increation {
                        println!("Port mouse enter: {port_type:?}, {}", port_name);
                        edge_in_creation
                            .set_end_port(
                                Some(EdgeCreationPort {
                                    node_id: node_id,
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
                let port_name = port_name.clone();
                let port_type = port_type.clone();
                move |event: MouseEvent| {
                    let edge_increation = editor_status.edge_in_creation.read().clone();
                    if let Some(mut edge_in_creation) = edge_increation {
                        println!("Port mouse enter: {port_type:?}, {}", port_name);
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
    // let port_w_h = 12.;
    // let border_radius = 1.;
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
