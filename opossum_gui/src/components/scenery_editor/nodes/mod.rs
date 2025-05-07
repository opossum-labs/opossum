use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use std::collections::HashMap;
pub mod nodes_component;

pub use nodes_component::Nodes;
use opossum_backend::{nodes::NodeInfo, AnalyzerType, NodeAttr, PortType};
use uuid::Uuid;

use super::{node::NodeElement, ports::ports_component::Ports, EDGES};
use crate::OPOSSUM_UI_LOGS;

#[derive(Clone, Copy, Eq, PartialEq, Default)]
pub struct NodesStore {
    optic_nodes: Signal<HashMap<Uuid, NodeElement>>,
    analyzer_nodes: Signal<HashMap<Uuid, AnalyzerType>>,
    active_node: Signal<Option<Uuid>>,
}

impl NodesStore {
    #[must_use]
    pub const fn optic_nodes(&self) -> Signal<HashMap<Uuid, NodeElement>> {
        self.optic_nodes
    }
    #[must_use]
    pub const fn analyzer_nodes(&self) -> Signal<HashMap<Uuid, AnalyzerType>> {
        self.analyzer_nodes
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

    pub fn shift_node_position(&mut self, node_id: &Uuid, shift: (f64, f64)) {
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
        // if let Some((z_index, idx)) = self.optic_nodes()()
        //     .iter()
        //     .position(|n| *n.id() == node_id)
        //     .map(|i| (self.optic_nodes().read().index(i).z_index(), i))
        // {
        //     OPOSSUM_UI_LOGS.write().add_log(&format!(
        //         "Removed node: {} (id:{node_id})",
        //         self.optic_nodes().read().index(idx).name()
        //     ));
        //     self.optic_nodes_mut().write().iter_mut().for_each(|n| {
        //         if *n.id() != node_id {
        //             n.set_inactive();
        //             if z_index <= n.z_index() {
        //                 n.set_z_index(n.z_index() - 1);
        //             }
        //         }
        //     });
        //     self.optic_nodes_mut().write().remove(idx);
        EDGES.write().remove_if_connected(node_id);
        //}
    }

    pub fn delete_nodes(&mut self) {
        self.optic_nodes_mut().write().clear();
        if self.nr_of_optic_nodes() == 0 {
            OPOSSUM_UI_LOGS.write().add_log("Removed all nodes!");
        } else {
            OPOSSUM_UI_LOGS.write().add_log("Error removing all nodes!");
        }
    }
    #[must_use]
    pub fn find_position(&self) -> (f64, f64) {
        let size = Point2D::new(130., 130. / 1.618_033_988_7);
        let mut new_pos = (size.x, size.x);
        let phi = 1. / 1.618_033_988_7;
        loop {
            let mut position_found = true;
            for node in self.optic_nodes().read().iter() {
                if (node.1.pos().0 - new_pos.0).abs() < 10.
                    && (node.1.pos().0 - new_pos.1).abs() < 10.
                {
                    position_found = false;
                    new_pos.0 += size.x * f64::powi(phi, 3);
                    new_pos.1 += size.y * f64::powi(phi, 3);
                    break;
                }
            }
            if position_found {
                break;
            }
        }
        new_pos
    }

    pub fn add_node(&mut self, node_info: &NodeInfo, node_attr: &NodeAttr) {
        let pos = self.find_position();
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
            .insert(*new_node.id(), new_node.clone());
        self.set_node_active(node_info.uuid(), new_node.z_index());
        OPOSSUM_UI_LOGS
            .write()
            .add_log(&format!("Added node: {}", node_info.node_type()));
    }

    pub fn add_analyzer(&mut self, analyzer: &AnalyzerType) {
        let pos = self.find_position();
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
    #[must_use]
    pub fn get_min_max_position(&self) -> (f64, f64, f64, f64) {
        let (mut min_x, mut min_y, mut max_y, mut max_x) = (0., 0., 0., 0.);
        for (idx, node) in self.optic_nodes().read().iter().enumerate() {
            if idx == 0 {
                min_x = node.1.pos().0;
                min_y = node.1.pos().1;
                max_y = node.1.pos().0;
                max_x = node.1.pos().1;
            } else {
                min_x = node.1.pos().0.min(min_x);
                min_y = node.1.pos().1.min(min_y);
                max_y = node.1.pos().0.max(max_y);
                max_x = node.1.pos().1.max(max_x);
            }
        }
        (min_x, min_y, max_y, max_x)
    }
}
