use std::collections::HashMap;

use crate::{
    api::{self},
    components::{
        context_menu::cx_menu::CxMenu,
        scenery_editor::{edges::define_bezier_path, graph_editor::graph_editor_component::EditorState},
    },
    CONTEXT_MENU, HTTP_API_CLIENT, OPOSSUM_UI_LOGS,
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_backend::{nodes::ConnectInfo, PortType};
use uuid::Uuid;

use super::edge::Edge;

// #[derive(Clone, PartialEq, Default)]
// pub struct Edges {
//     edges: HashMap<String, Edge>,
// }
// impl Edges {
//     #[must_use]
//     pub fn new() -> Self {
//         Self {
//             edges: HashMap::new(),
//         }
//     }
//     pub fn add_edge(&mut self, edge: Edge) {
//         self.edges.insert(edge.start_port_id(), edge);
//     }
//     #[must_use]
//     pub fn get_edge(&self, id: &str) -> Option<&Edge> {
//         self.edges.get(id)
//     }
//     #[must_use]
//     pub fn get_edge_mut(&mut self, id: &str) -> Option<&mut Edge> {
//         self.edges.get_mut(id)
//     }
//     #[must_use]
//     pub const fn edges(&self) -> &HashMap<String, Edge> {
//         &self.edges
//     }
//     #[must_use]
//     pub const fn edges_mut(&mut self) -> &mut HashMap<String, Edge> {
//         &mut self.edges
//     }

//     pub fn shift_if_connected(&mut self, x_shift: f64, y_shift: f64, node_id: Uuid) {
//         for edge in self.edges_mut().values_mut() {
//             edge.shift_if_connected(x_shift, y_shift, node_id);
//         }
//     }
//     pub fn remove_if_connected(&mut self, node_id: Uuid) {
//         let keys_to_remove = self
//             .edges()
//             .iter()
//             .filter(|(_, edge)| edge.is_connected(node_id))
//             .map(|(k, _)| k.clone())
//             .collect::<Vec<String>>();

//         for key in &keys_to_remove {
//             self.edges_mut().remove(key);
//         }
//     }
//     pub fn remove_edge(&mut self, conn_info: &ConnectInfo) {
//         self.edges.remove(&format!(
//             "{}_{}",
//             conn_info.src_uuid().as_simple(),
//             conn_info.src_port()
//         ));
//     }
// }

#[derive(Clone, Debug)]
pub struct NewEdgeCreationStart {
    pub src_node: Uuid,
    pub src_port: String,
    pub src_port_type: PortType,
    pub start_pos: Point2D<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeCreationPort {
    pub node_id: Uuid,
    pub port_name: String,
    pub port_type: PortType,
}
#[derive(Clone, PartialEq, Debug)]
pub struct EdgeCreation {
    start_port: EdgeCreationPort,
    end_port: Option<EdgeCreationPort>,
    start: Point2D<f64>,
    end: Point2D<f64>,
    bezier_offset: f64,
}

impl EdgeCreation {
    #[must_use]
    pub fn new(
        src_node: Uuid,
        src_port: String,
        src_port_type: PortType,
        start: Point2D<f64>,
    ) -> Self {
        let connection_factor = if src_port_type == PortType::Input {
            -1.
        } else {
            1.
        };
        Self {
            start_port: EdgeCreationPort {
                node_id: src_node,
                port_name: src_port.clone(),
                port_type: src_port_type,
            },
            end_port: None,
            start,
            end: start,
            bezier_offset: 50. * connection_factor,
        }
    }
    #[must_use]
    pub const fn start(&self) -> Point2D<f64> {
        self.start
    }
    #[must_use]
    pub const fn end(&self) -> Point2D<f64> {
        self.end
    }
    pub fn shift_end(&mut self, shift: Point2D<f64>) {
        self.end.x += shift.x;
        self.end.y += shift.y;
    }
    #[must_use]
    pub const fn bezier_offset(&self) -> f64 {
        self.bezier_offset
    }
    pub fn set_end_port(&mut self, end_port: Option<EdgeCreationPort>) {
        self.end_port = end_port;
    }
    pub fn is_valid(&self) -> bool {
        if let Some(end_port) = &self.end_port {
            if end_port.node_id == self.start_port.node_id {
                return false;
            }
            if end_port.port_type == self.start_port.port_type {
                return false;
            }
            return true;
        } else {
            return false;
        }
    }
}


#[component]
pub fn EdgesComponent() -> Element {
    rsx! {
        // for edge in EDGES.read().edges().values() {
        //     // EdgeComponent { edge: edge.clone() }
        // }
    }
}

#[component]
pub fn EdgeCreationComponent() -> Element {
    let editor_status = use_context::<EditorState>();
    let edge_in_creation = &*(editor_status.edge_in_creation.read());
    edge_in_creation.clone().map_or_else(
        || rsx! {},
        |edge| {
            let new_path = define_bezier_path(edge.start(), edge.end(), edge.bezier_offset());
            rsx! {
                path {
                    d: new_path,
                    stroke: "black",
                    fill: "transparent",
                    stroke_width: format!("{}", 2.),
                }
            }
        },
    )
}

