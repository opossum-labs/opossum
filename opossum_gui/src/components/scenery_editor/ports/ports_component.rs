use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_backend::{usize_to_f64, PortType};
use uuid::Uuid;

use crate::{
    api::{self},
    components::scenery_editor::{
        edges::edges_component::{Edge, EdgeCreation, NewEdgeCreationStart},
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
    port_w_h: f64,
    port_x: f64,
    port_y: f64,
    node_id: Uuid,
    port_name: String,
    port_type: PortType,
) -> Element {
    // let mut edge_in_creation = use_context::<Signal<Option<EdgeCreation>>>();
    let mut editor_status = use_context::<EditorState>();
    // let on_mouse_down = {
    //     let port_type = port_type.clone();
    //     let port_name = port_name.clone();
    //     move |e: Event<MouseData>| {
    //         let x = (-e.element_coordinates().x + port_w_h / 2.)
    //             .mul_add(zoom_factor, e.page_coordinates().x);
    //         let y = (-e.element_coordinates().y + port_w_h / 2.)
    //             .mul_add(zoom_factor, e.page_coordinates().y);
    //         let start_end = Point2D::new(x, y);
    //         edge_in_creation.set(Some(EdgeCreation::new(
    //             node_id,
    //             port_name.clone(),
    //             port_type.clone(),
    //             start_end,
    //             start_end,
    //             70. * zoom_factor,
    //         )));
    //     }
    // };

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
    let port_name_clone = port_name.clone();
    rsx! {
        div {
            onmousedown: move |event: MouseEvent| {
                println!("Port mouse down: {port_type:?}, {}", port_name_clone);
                editor_status.drag_status.set(DragStatus::Edge(NewEdgeCreationStart{
                    src_node: node_id,
                    src_port: port_name_clone.clone(),
                    src_port_type: port_type.clone(),
                    start_pos: Point2D::new(
                        port_x,
                        port_y,
                    ),
                    bezier_offset: 70.,
                }));
                event.stop_propagation();
            },
            // onmouseup: on_mouse_up,
            id: format!("{}_{}", node_id.as_simple().to_string(), port_name),
            class: "port {port_class}",
            style: format!(
                "left: {}px; top: {}px; width: {}px; height: {}px;",
                port_x,
                port_y,
                port_w_h,
                port_w_h,
            ),
        }
    }
}

#[component]
pub fn NodePorts(
    node_width: f64,
    node_height: f64,
    node_id: Uuid,
    input_ports: Vec<String>,
    output_ports: Vec<String>,
) -> Element {
    let port_w_h = 12.;
    let border_radius = 1.;

    rsx! {
        for (i , in_port) in input_ports.iter().enumerate() {
            {
                let port_y = usize_to_f64(i)
                    .mul_add(20., node_height / 2. - port_w_h / 2. - border_radius);
                let port_x = -port_w_h / 2. - 3. * border_radius / 2.;
                rsx! {
                    NodePort {
                        port_w_h,
                        port_x,
                        port_y,
                        node_id,
                        port_name: in_port.clone(),
                        port_type: PortType::Input,
                    }
                }
            }
        }
        for (i , out_port) in output_ports.iter().enumerate() {
            {
                let port_y = usize_to_f64(i)
                    .mul_add(20., node_height / 2. - port_w_h / 2. - border_radius);
                let port_x = node_width - port_w_h / 2. - border_radius / 2.;
                rsx! {
                    NodePort {
                        port_w_h,
                        port_x,
                        port_y,
                        node_id,
                        port_name: out_port.clone(),
                        port_type: PortType::Output,
                    }
                }
            }
        }
    }
}
