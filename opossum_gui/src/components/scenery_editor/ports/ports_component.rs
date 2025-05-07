use dioxus::{
    html::geometry::{euclid::default::Point2D, PixelsSize},
    prelude::*,
};
use opossum_backend::{usize_to_f64, PortType};
use uuid::Uuid;

use crate::{
    api::{self},
    components::scenery_editor::{
        edges::edges_component::{EdgeCreation, EdgeCreationPort, NewEdgeCreationStart},
        graph_editor::graph_editor_component::{DragStatus, EditorState},
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
pub fn NodePort(
    node_body_position: Point2D<f64>,
    port_w_h: f64,
    port_pos: Point2D<f64>,
    node_id: Uuid,
    port_name: String,
    port_type: PortType,
) -> Element {
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
    let port_class = if port_type == PortType::Input {
        "input-port"
    } else {
        "output-port"
    };
    rsx! {
        div {
            id: format!("{}_{}", node_id.as_simple().to_string(), port_name),
            class: "port {port_class}",
            style: format!(
                "left: {}px; top: {}px; width: {}px; height: {}px;",
                port_pos.x,
                port_pos.y,
                port_w_h,
                port_w_h,
            ),
            draggable: false,
            onmousedown: {
                let port_name = port_name.clone();
                let port_type = port_type.clone();
                move |event: MouseEvent| {
                    let start_pos = Point2D::new(
                        port_pos.x + node_body_position.x + port_w_h / 2.0,
                        port_pos.y + node_body_position.y + port_w_h / 2.0,
                    );
                    editor_status
                        .drag_status
                        .set(
                            DragStatus::Edge(NewEdgeCreationStart {
                                src_node: node_id,
                                src_port: port_name.clone(),
                                src_port_type: port_type.clone(),
                                start_pos: start_pos
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
pub fn NodePorts(
    node_body_position: Point2D<f64>,
    node_size: PixelsSize,
    node_id: Uuid,
    ports: Ports,
) -> Element {
    let port_w_h = 12.;
    let border_radius = 1.;

    rsx! {
        for (i , in_port) in ports.input_ports().iter().enumerate() {
            {
                let port_y = usize_to_f64(i)
                    .mul_add(20., node_size.height / 2. - port_w_h / 2. - border_radius);
                let port_x = -port_w_h / 2. - 3. * border_radius / 2.;
                rsx! {
                    NodePort {
                        node_body_position,
                        port_w_h,
                        port_pos: Point2D::new(port_x, port_y),
                        node_id,
                        port_name: in_port.clone(),
                        port_type: PortType::Input,
                    }
                }
            }
        }
        for (i , out_port) in ports.output_ports().iter().enumerate() {
            {
                let port_y = usize_to_f64(i)
                    .mul_add(20., node_size.height / 2. - port_w_h / 2. - border_radius);
                let port_x = node_size.width - port_w_h / 2. - border_radius / 2.;
                rsx! {
                    NodePort {
                        node_body_position,
                        port_w_h,
                        port_pos: Point2D::new(port_x, port_y),
                        node_id,
                        port_name: out_port.clone(),
                        port_type: PortType::Output,
                    }
                }
            }
        }
    }
}
