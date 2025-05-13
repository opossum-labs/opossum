use super::{
    node::{NodeElement, NodeType, HEADER_HEIGHT, NODE_WIDTH},
    ports::ports_component::{NodePorts, Ports},
};
use crate::{
    api::{self, http_client::HTTPClient},
    HTTP_API_CLIENT, OPOSSUM_UI_LOGS,
};
use dioxus::{
    html::geometry::euclid::{
        default::{Point2D, Rect},
        Size2D,
    },
    prelude::*,
};
use opossum_backend::{
    nodes::{ConnectInfo, NewNode},
    scenery::NewAnalyzerInfo,
    PortType,
};
use std::{collections::HashMap, fs, path::Path};
use uuid::Uuid;

#[derive(Clone, Copy, Eq, PartialEq, Default)]
pub struct GraphStore {
    nodes: Signal<HashMap<Uuid, NodeElement>>,
    edges: Signal<Vec<ConnectInfo>>,
    active_node: Signal<Option<Uuid>>,
}
impl GraphStore {
    pub async fn load_from_opm_file(&mut self, path: &Path) {
        let opm_string = fs::read_to_string(path);
        match opm_string {
            Ok(opm_string) => {
                match api::post_opm_file(&HTTP_API_CLIENT(), opm_string).await {
                    Ok(_) => {
                        match api::get_nodes(&HTTP_API_CLIENT()).await {
                            Ok(nodes) => {
                                self.nodes()().clear();
                                self.edges()().clear();
                                // let mut active_node = self.active_node.write();
                                // *active_node = None;
                                let node_elements: Vec<NodeElement> = nodes.iter().map(|node| {  
                                    let position = if let Some(position) = node.gui_position() {
                                        Point2D::new(position.0, position.1)
                                    } else {
                                        Point2D::zero()
                                    };
                                    //let ports=self.get_ports(node.uuid()).await;
                                    NodeElement::new(
                                        NodeType::Optical(node.node_type().to_string()),
                                        node.uuid(),
                                        position,
                                        Ports::default(),
                                    )
                                }).collect();
                                for node_element in node_elements {
                                    self.nodes()().insert(node_element.id(), node_element);
                                }
                            }
                            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
                        }
                        // Update graph store
                    }
                    Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
                }
            }
            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str.to_string()),
        }
    }
    pub async fn save_to_opm_file(&self, path: &Path) {
        match api::get_opm_file(&HTTP_API_CLIENT()).await {
            Ok(opm_string) => {
                if let Err(err_str) = fs::write(path, opm_string) {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str.to_string())
                }
            }
            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
        }
    }
    #[must_use]
    pub const fn nodes(&self) -> Signal<HashMap<Uuid, NodeElement>> {
        self.nodes
    }
    #[must_use]
    pub const fn edges(&self) -> Signal<Vec<ConnectInfo>> {
        self.edges
    }
    #[must_use]
    pub const fn edges_mut(&mut self) -> &mut Signal<Vec<ConnectInfo>> {
        &mut self.edges
    }
    pub const fn nodes_mut(&mut self) -> &mut Signal<HashMap<Uuid, NodeElement>> {
        &mut self.nodes
    }
    pub fn shift_node_position(&mut self, node_id: &Uuid, shift: Point2D<f64>) {
        if let Some(node) = self.nodes_mut().write().get_mut(node_id) {
            node.shift_position(shift);
        }
    }
    #[must_use]
    pub fn active_node(&self) -> Option<Uuid> {
        self.active_node.read().clone()
    }
    pub fn set_node_active(&mut self, id: Uuid) {
        let mut active_node = self.active_node.write();
        *active_node = Some(id);
    }
    pub async fn delete_node(&mut self, node_id: Uuid) {
        let nodes = self.nodes()();
        if let Some(node_element) = nodes.get(&node_id) {
            match node_element.node_type() {
                NodeType::Optical(_) => self.delete_optical_node(node_id).await,
                NodeType::Analyzer(_) => self.delete_analyzer_node(node_id).await,
            }
        }
    }
    async fn delete_optical_node(&mut self, node_id: Uuid) {
        match api::delete_node(&HTTP_API_CLIENT(), node_id).await {
            Ok(deleted_ids) => {
                for node_id in deleted_ids {
                    self.nodes_mut().write().remove(&node_id);
                    // remove all edges no longer valid
                    let mut edges = self.edges()();
                    edges.retain_mut(|e| e.src_uuid() != node_id && e.target_uuid() != node_id);
                    *self.edges().write() = edges;
                }
            }
            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
        }
    }
    async fn delete_analyzer_node(&mut self, node_id: Uuid) {
        match api::delete_analyzer(&HTTP_API_CLIENT(), node_id).await {
            Ok(_) => {
                self.nodes_mut().write().remove(&node_id);
            }
            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
        }
    }
    pub async fn delete_edge(&self, edge: ConnectInfo) {
        match api::delete_connection(&HTTP_API_CLIENT(), edge.clone()).await {
            Ok(_connect_info) => {
                let i = self.edges()().iter().position(|e| {
                    e.src_uuid() == edge.src_uuid()
                        && e.src_port() == edge.src_port()
                        && e.target_uuid() == edge.target_uuid()
                        && e.target_port() == edge.target_port()
                });
                if let Some(index) = i {
                    self.edges().write().remove(index);
                }
            }
            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
        }
    }
    pub async fn update_edge(&mut self, edge: &ConnectInfo) {
        match api::update_distance(&HTTP_API_CLIENT(), edge.clone()).await {
            Ok(_) => {
                let i = self.edges()().iter().position(|e| {
                    e.src_uuid() == edge.src_uuid()
                        && e.src_port() == edge.src_port()
                        && e.target_uuid() == edge.target_uuid()
                        && e.target_port() == edge.target_port()
                });
                if let Some(index) = i {
                    let mut edges = self.edges_mut().write();
                    edges[index] = edge.clone();
                }
            }
            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
        }
    }
    pub async fn delete_all_nodes(&mut self) {
        match api::delete_scenery(&HTTP_API_CLIENT()).await {
            Ok(_) => {
                self.nodes_mut().write().clear();
                self.edges_mut().write().clear();
            }
            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
        }
    }
    pub async fn add_edge(&mut self, edge: ConnectInfo) {
        match api::post_add_connection(&HTTP_API_CLIENT(), edge.clone()).await {
            Ok(_) => {
                self.edges_mut().write().push(edge);
            }
            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
        }
    }
    pub fn get_bounding_box(&self) -> Rect<f64> {
        let optic_nodes = self.nodes()();
        if optic_nodes.is_empty() {
            return Rect::new(Point2D::zero(), Size2D::zero());
        }
        let node = optic_nodes.iter().next().unwrap().1;
        let mut min_x = node.pos().x;
        let mut min_y = node.pos().y;
        let mut max_x = node.pos().x + NODE_WIDTH;
        let mut max_y = node.pos().y + HEADER_HEIGHT + node.node_body_height();
        for node in optic_nodes {
            let x = node.1.pos().x;
            let y = node.1.pos().y;
            if min_x > x {
                min_x = x;
            }
            if min_y > y {
                min_y = y;
            }
            if max_x < x {
                max_x = x;
            }
            if max_y < y {
                max_y = y;
            }
        }
        Rect::new(
            Point2D::new(min_x, min_y),
            Size2D::new(max_x - min_y, max_y - min_y),
        )
    }
    // #[must_use]
    // pub fn find_position(&self) -> Point2D<f64> {
    //     let size = Point2D::new(130., 130. / 1.618_033_988_7);
    //     let mut new_pos = Point2D::new(size.x, size.x);
    //     let phi = 1. / 1.618_033_988_7;
    //     loop {
    //         let mut position_found = true;
    //         for node in self.optic_nodes().read().iter() {
    //             if (node.1.pos().x - new_pos.x).abs() < 10.
    //                 && (node.1.pos().y - new_pos.y).abs() < 10.
    //             {
    //                 position_found = false;
    //                 new_pos.x += size.x * f64::powi(phi, 3);
    //                 new_pos.y += size.y * f64::powi(phi, 3);
    //                 break;
    //             }
    //         }
    //         if position_found {
    //             break;
    //         }
    //     }
    //     new_pos
    // }
    async fn get_ports(&self, node_id: Uuid) -> Ports {
        match api::get_node_properties(&HTTP_API_CLIENT(), node_id).await {
            Ok(node_attr) => {
                let input_ports = node_attr
                    .ports()
                    .ports(&PortType::Input)
                    .keys()
                    .cloned()
                    .collect::<Vec<String>>();
                let output_ports = node_attr
                    .ports()
                    .ports(&PortType::Output)
                    .keys()
                    .cloned()
                    .collect::<Vec<String>>();
                Ports::new(input_ports, output_ports)
            }
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                Ports::default()
            }
        }
    }
    pub async fn add_optic_node(&mut self, new_node_info: NewNode) {
        match api::post_add_node(&HTTP_API_CLIENT(), new_node_info, Uuid::nil()).await {
            Ok(node_info) => {
                let ports = self.get_ports(node_info.uuid()).await;
                let new_node = NodeElement::new(
                    NodeType::Optical(node_info.name().to_string()),
                    node_info.uuid(),
                    Point2D::new(100.0, 100.0),
                    ports,
                );
                self.nodes_mut().write().insert(new_node.id(), new_node);
                self.set_node_active(node_info.uuid());
            }
            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
        }
    }

    pub async fn add_analyzer(&mut self, new_analyzer_info: NewAnalyzerInfo) {
        match api::post_add_analyzer(&HTTP_API_CLIENT(), new_analyzer_info.clone()).await {
            Ok(analyzer_id) => {
                let new_node = NodeElement::new(
                    NodeType::Analyzer(new_analyzer_info.analyzer_type.clone()),
                    analyzer_id,
                    Point2D::new(
                        new_analyzer_info.clone().gui_position.0 as f64,
                        new_analyzer_info.gui_position.1 as f64,
                    ),
                    Ports::new(vec![], vec![]),
                );
                self.nodes_mut().write().insert(analyzer_id, new_node);
            }
            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
        }
    }
}
