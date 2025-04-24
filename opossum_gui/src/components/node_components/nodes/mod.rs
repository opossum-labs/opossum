use std::{collections::HashMap, ops::Index};

use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
pub mod nodes_component;

use crate::opm_types::*;
pub use nodes_component::Nodes;
use uuid::Uuid;

use crate::{api, ACTIVE_NODE, EDGES, HTTP_API_CLIENT, OPOSSUM_UI_LOGS, ZOOM};

use super::{
    node_drag_drop_container::drag_drop_container::ZoomShift, ports::ports_component::Ports,
    NodeElement, NodeOffset,
};

#[derive(Clone, Eq, PartialEq, Default)]
pub struct NodesStore {
    optic_nodes: Signal<Vec<NodeElement>>,
    analyzer_nodes: Signal<HashMap<Uuid, AnalyzerType>>,
}

impl NodesStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            optic_nodes: Signal::new(Vec::<NodeElement>::new()),
            analyzer_nodes: Signal::new(HashMap::<Uuid, AnalyzerType>::new()),
        }
    }
    #[must_use]
    pub fn size() -> Point2D<f64> {
        Point2D::new(130., 130. / 1.618_033_988_7)
    }
    #[must_use]
    pub const fn optic_nodes(&self) -> Signal<Vec<NodeElement>> {
        self.optic_nodes
    }
    #[must_use]
    pub const fn analyzer_nodes(&self) -> Signal<HashMap<Uuid, AnalyzerType>> {
        self.analyzer_nodes
    }
    pub const fn optic_nodes_mut(&mut self) -> &mut Signal<Vec<NodeElement>> {
        &mut self.optic_nodes
    }

    pub const fn analyzer_nodes_mut(&mut self) -> &mut Signal<HashMap<Uuid, AnalyzerType>> {
        &mut self.analyzer_nodes
    }
    #[must_use]
    pub fn nr_of_optic_nodes(&self) -> usize {
        self.optic_nodes().read().len()
    }
    #[must_use]
    pub fn available_analyzers() -> Vec<AnalyzerType> {
        vec![
            AnalyzerType::Energy,
            AnalyzerType::RayTrace(RayTraceConfig::default()),
            AnalyzerType::GhostFocus(GhostFocusConfig::default()),
        ]
    }

    pub fn drag_node(&mut self, node_id: &Uuid, elem_offset: &(f64, f64), mouse_data: &MouseData) {
        let offset = use_context::<Signal<NodeOffset>>();
        if let Some(node_container_offset) = offset().offset() {
            let coordinates = mouse_data.page_coordinates();
            if let Some(n) = self
                .optic_nodes_mut()
                .write()
                .iter_mut()
                .find(|n| n.id() == node_id)
            {
                n.drag_node(
                    coordinates.x,
                    coordinates.y,
                    node_container_offset.0,
                    node_container_offset.1,
                    elem_offset.0,
                    elem_offset.1,
                );
            };
        }
    }
    #[must_use]
    pub fn get_active_node_id(&self) -> Option<Uuid> {
        self.optic_nodes()
            .read()
            .iter()
            .find(|n| n.is_active())
            .map(|n| *n.id())
    }

    pub fn set_node_active(&mut self, id: Uuid, z_index: usize) {
        let nr_of_nodes = self.nr_of_optic_nodes();
        self.optic_nodes_mut().write().iter_mut().for_each(|n| {
            if *n.id() == id {
                n.set_active();
                n.set_z_index(nr_of_nodes);
            } else {
                n.set_inactive();
                if z_index <= n.z_index() {
                    n.set_z_index(n.z_index() - 1);
                }
            }
        });

        spawn(async move {
            if let Ok(node_attr) = api::get_node_properties(&HTTP_API_CLIENT(), id).await {
                let mut active_node = ACTIVE_NODE.write();
                *active_node = Some(node_attr);
            }
        });
    }

    pub fn deactivate_nodes(&mut self) {
        self.optic_nodes_mut()
            .write()
            .iter_mut()
            .for_each(NodeElement::set_inactive);
    }

    pub fn delete_node(&mut self, node_id: Uuid) {
        if let Some((z_index, idx)) = self.optic_nodes()()
            .iter()
            .position(|n| *n.id() == node_id)
            .map(|i| (self.optic_nodes().read().index(i).z_index(), i))
        {
            OPOSSUM_UI_LOGS.write().add_log(&format!(
                "Removed node: {} (id:{node_id})",
                self.optic_nodes().read().index(idx).name()
            ));
            self.optic_nodes_mut().write().iter_mut().for_each(|n| {
                if *n.id() != node_id {
                    n.set_inactive();
                    if z_index <= n.z_index() {
                        n.set_z_index(n.z_index() - 1);
                    }
                }
            });
            self.optic_nodes_mut().write().remove(idx);
            EDGES.write().remove_if_connected(node_id);
        }
    }

    pub fn delete_nodes(&mut self) {
        let ids = self
            .optic_nodes()
            .read()
            .iter()
            .map(|n| *n.id())
            .collect::<Vec<Uuid>>();
        for node_id in &ids {
            self.delete_node(*node_id);
        }
        if self.nr_of_optic_nodes() == 0 {
            OPOSSUM_UI_LOGS.write().add_log("Removed all nodes!");
        } else {
            OPOSSUM_UI_LOGS.write().add_log("Error removing all nodes!");
        }
    }
    #[must_use]
    pub fn find_position(&self) -> Point2D<f64> {
        let zoom_factor = ZOOM.read().zoom_factor();
        let size = Self::size();
        let mut new_x = size.x * zoom_factor;
        let mut new_y = size.x * zoom_factor;
        let phi = 1. / 1.618_033_988_7;
        loop {
            let mut position_found = true;
            for node in self.optic_nodes().read().iter() {
                if (node.x() - new_x).abs() < 10. && (node.y() - new_y).abs() < 10. {
                    position_found = false;
                    new_x += size.x * f64::powi(phi, 3) * zoom_factor;
                    new_y += size.y * f64::powi(phi, 3) * zoom_factor;
                    break;
                }
            }
            if position_found {
                break;
            }
        }
        Point2D::new(new_x, new_y)
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
            pos.x,
            pos.y,
            node_info.uuid(),
            node_info.name().to_string(),
            false,
            self.nr_of_optic_nodes(),
            Ports::new(input_ports, output_ports),
        );
        self.optic_nodes_mut().write().push(new_node.clone());
        self.set_node_active(node_info.uuid(), new_node.z_index());
        OPOSSUM_UI_LOGS
            .write()
            .add_log(&format!("Added node: {}", node_info.node_type()));
    }

    pub fn add_analyzer(&mut self, analyzer: &AnalyzerType) {
        let pos = self.find_position();

        let new_node = NodeElement::new(
            pos.x,
            pos.y,
            Uuid::new_v4(),
            format!("{analyzer}"),
            false,
            self.nr_of_optic_nodes(),
            Ports::new(vec![], vec![]),
        );

        self.optic_nodes_mut().write().push(new_node);
    }
    #[must_use]
    pub fn get_min_max_position(&self) -> (f64, f64, f64, f64) {
        let (mut min_x, mut min_y, mut max_y, mut max_x) = (0., 0., 0., 0.);
        for (idx, node) in self.optic_nodes().read().iter().enumerate() {
            if idx == 0 {
                min_x = node.x();
                min_y = node.y();
                max_y = node.y();
                max_x = node.x();
            } else {
                min_x = node.x().min(min_x);
                min_y = node.y().min(min_y);
                max_y = node.y().max(max_y);
                max_x = node.x().max(max_x);
            }
        }

        (min_x, min_y, max_y, max_x)
    }
}

impl ZoomShift for NodesStore {
    fn zoom_shift(&mut self, zoom_factor: f64, shift: (f64, f64), zoom_center: (f64, f64)) {
        self.optic_nodes_mut().write().iter_mut().for_each(|n| {
            n.zoom_shift(zoom_factor, shift, zoom_center);
        });
    }
}
