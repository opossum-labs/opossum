use std::collections::HashMap;

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
use uuid::Uuid;

use crate::{api, HTTP_API_CLIENT, OPOSSUM_UI_LOGS};

use super::{
    node::{NodeElement, NodeType, HEADER_HEIGHT, NODE_WIDTH},
    ports::ports_component::Ports,
};

#[derive(Clone, Copy, Eq, PartialEq, Default)]
pub struct GraphStore {
    nodes: Signal<HashMap<Uuid, NodeElement>>,
    edges: Signal<Vec<ConnectInfo>>,
    active_node: Signal<Option<Uuid>>,
}

impl GraphStore {
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
    #[must_use]
    pub fn nr_of_optic_nodes(&self) -> usize {
        self.nodes().read().len()
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
    pub async fn delete_edge(&mut self, edge: ConnectInfo) {
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
                min_x = x
            }
            if min_y > y {
                min_y = y
            }
            if max_x < x {
                max_x = x
            }
            if max_y < y {
                max_y = y
            }
        }
        return Rect::new(
            Point2D::new(min_x, min_y),
            Size2D::new(max_x - min_y, max_y - min_y),
        );
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

    pub async fn add_optic_node(&mut self, new_node_info: NewNode) {
        match api::post_add_node(&HTTP_API_CLIENT(), new_node_info, Uuid::nil()).await {
            Ok(node_info) => {
                match api::get_node_properties(&HTTP_API_CLIENT(), node_info.uuid()).await {
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
                        let new_node = NodeElement::new(
                            NodeType::Optical(node_info.name().to_string()),
                            node_info.uuid(),
                            Point2D::new(100.0, 100.0),
                            Ports::new(input_ports, output_ports),
                        );
                        self.nodes_mut()
                            .write()
                            .insert(new_node.id(), new_node.clone());
                        self.set_node_active(node_info.uuid());
                    }
                    Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
                }
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
