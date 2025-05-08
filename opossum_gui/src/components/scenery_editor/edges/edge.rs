use dioxus::html::geometry::euclid::default::Point2D;
use opossum_backend::PortType;

use crate::components::scenery_editor::node::NodeElement;

#[derive(Clone, PartialEq)]
pub struct Edge {
    src_node: NodeElement,
    target_node: NodeElement,
    src_port: String,
    target_port: String,
    distance: f64,
}
impl Edge {
    #[must_use]
    pub const fn new(
        src_node: NodeElement,
        target_node: NodeElement,
        src_port: String,
        target_port: String,
        distance: f64,
    ) -> Self {
        Self {
            src_node,
            target_node,
            src_port,
            target_port,
            distance
        }
    }
    #[must_use]
    pub fn start_position(&self) -> Point2D<f64> {
       self.src_node.abs_port_position(&PortType::Output, &self.src_port)
    }
    #[must_use]
    pub fn end_position(&self) -> Point2D<f64> {
        self.target_node.abs_port_position(&PortType::Input, &self.target_port)
    }
}
