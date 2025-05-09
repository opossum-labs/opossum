use std::collections::HashMap;

use dioxus::{
    html::geometry::euclid::{
        default::{Point2D, Rect},
        Size2D,
    },
    prelude::*,
};
use opossum_backend::{nodes::NodeInfo, AnalyzerType, NodeAttr, PortType};
use uuid::Uuid;

use crate::OPOSSUM_UI_LOGS;

use super::{
    edges::edge::Edge,
    node::{NodeElement, HEADER_HEIGHT, NODE_WIDTH},
    ports::ports_component::Ports,
};

#[derive(Clone, Copy, Eq, PartialEq, Default)]
pub struct GraphStore {
    optic_nodes: Signal<HashMap<Uuid, NodeElement>>,
    edges: Signal<Vec<Edge>>,
    analyzer_nodes: Signal<HashMap<Uuid, AnalyzerType>>,
    active_node: Signal<Option<Uuid>>,
}

impl GraphStore {
    #[must_use]
    pub const fn optic_nodes(&self) -> Signal<HashMap<Uuid, NodeElement>> {
        self.optic_nodes
    }
    #[must_use]
    pub const fn analyzer_nodes(&self) -> Signal<HashMap<Uuid, AnalyzerType>> {
        self.analyzer_nodes
    }
    #[must_use]
    pub const fn edges(&self) -> Signal<Vec<Edge>> {
        self.edges
    }
    #[must_use]
    pub const fn edges_mut(&mut self) -> &mut Signal<Vec<Edge>> {
        &mut self.edges
    }
    pub const fn optic_nodes_mut(&mut self) -> &mut Signal<HashMap<Uuid, NodeElement>> {
        &mut self.optic_nodes
    }

    pub const fn analyzer_nodes_mut(&mut self) -> &mut Signal<HashMap<Uuid, AnalyzerType>> {
        &mut self.analyzer_nodes
    }
    #[must_use]
    pub fn nr_of_optic_nodes(&self) -> usize {
        self.optic_nodes().read().len()
    }

    pub fn shift_node_position(&mut self, node_id: &Uuid, shift: Point2D<f64>) {
        if let Some(node) = self.optic_nodes_mut().write().get_mut(node_id) {
            node.shift_position(shift);
        }
    }
    #[must_use]
    pub fn active_node(&self) -> Option<Uuid> {
        self.active_node.read().clone()
    }

    pub fn set_node_active(&mut self, id: Uuid, z_index: usize) {
        let mut active_node = self.active_node.write();
        *active_node = Some(id);
        // let nr_of_nodes = self.nr_of_optic_nodes();
        // self.optic_nodes_mut().write().iter_mut().for_each(|n| {
        //     if *n.0 == id {
        //         n.1.set_z_index(nr_of_nodes);
        //     } else {
        //         if z_index <= n.1.z_index() {
        //             n.1.set_z_index(n.1.z_index() - 1);
        //         }
        //     }
        // });

        // spawn(async move {
        //     if let Ok(node_attr) = api::get_node_properties(&HTTP_API_CLIENT(), id).await {
        //         let mut active_node = ACTIVE_NODE.write();
        //         *active_node = Some(node_attr);
        //     }
        // });
    }
    pub fn deactivate_nodes(&mut self) {
        let mut active_node = self.active_node.write();
        *active_node = None;
    }
    pub fn delete_node(&mut self, node_id: Uuid) {
        self.optic_nodes_mut().write().remove(&node_id);
        // remove all edges no longer valid
        let mut edges = self.edges()();
        edges.retain_mut(|e| e.src_port().node_id != node_id && e.target_port().node_id != node_id);
        *self.edges().write() = edges;
        OPOSSUM_UI_LOGS
            .write()
            .add_log(&format!("Removed node: {node_id}"));
    }
    pub fn delete_edge(&mut self, edge: &Edge) {
        let edges = self.edges()();
        let i = edges.iter().position(|e| {
            e.src_port().node_id == edge.src_port().node_id
                && e.src_port().port_name == edge.src_port().port_name
                && e.target_port().node_id == edge.target_port().node_id
                && e.target_port().port_name == edge.target_port().port_name
        });
        if let Some(index) = i {
            self.edges().write().remove(index);
        }
    }
    pub fn delete_all_nodes(&mut self) {
        self.optic_nodes_mut().write().clear();
        self.edges_mut().write().clear();
        OPOSSUM_UI_LOGS.write().add_log("Removed all nodes!");
    }
    pub fn get_bounding_box(&self) -> Rect<f64> {
        let optic_nodes = self.optic_nodes()();
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

    pub fn add_node(&mut self, node_info: &NodeInfo, node_attr: &NodeAttr) {
        let pos = Point2D::new(100.0, 100.0);
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
            node_info.name().to_string(),
            node_info.uuid(),
            pos,
            self.nr_of_optic_nodes(),
            Ports::new(input_ports, output_ports),
        );
        self.optic_nodes_mut()
            .write()
            .insert(new_node.id(), new_node.clone());
        self.set_node_active(node_info.uuid(), new_node.z_index());
        OPOSSUM_UI_LOGS
            .write()
            .add_log(&format!("Added node: {}", node_info.node_type()));
    }

    pub fn add_analyzer(&mut self, analyzer: &AnalyzerType) {
        let pos = Point2D::new(100.0, 100.0);
        let id = Uuid::new_v4();
        let new_node = NodeElement::new(
            format!("{analyzer}"),
            id,
            pos,
            self.nr_of_optic_nodes(),
            Ports::new(vec![], vec![]),
        );
        self.optic_nodes_mut().write().insert(id, new_node);
    }
}
