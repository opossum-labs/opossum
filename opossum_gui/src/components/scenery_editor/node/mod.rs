use dioxus::html::geometry::euclid::default::Point2D;
use opossum_backend::{usize_to_f64, AnalyzerType, PortType};
use uuid::Uuid;
mod graph_node_components;
pub mod node_component;
pub use node_component::Node;

use super::ports::ports_component::Ports;

// Constants for node dimensions and port positions
const GOLDEN_RATIO: f64 = 1.6180339887;
// The node width is fixed, but the height is dynamic
// depending on the number of ports
pub const NODE_WIDTH: f64 = 130.0;
// The header height is fixed
pub const HEADER_HEIGHT: f64 = 30.0;
// The minimum node body height is fixed such that the overall node height (header + body) corresponds to
// to the golden ratio
pub const MIN_NODE_BODY_HEIGHT: f64 = NODE_WIDTH / GOLDEN_RATIO - HEADER_HEIGHT;
// Nodes with only one port will be vertically centered
// in the node body, so we need to add some padding
pub const PORT_VER_PADDING: f64 = MIN_NODE_BODY_HEIGHT / 2.0;
// The vertical spacing between ports is fixed
pub const PORT_VER_SPACING: f64 = 16.0;
pub const PORT_HEIGHT: f64 = 12.0;
pub const PORT_WIDTH: f64 = 12.0;

#[derive(Clone, PartialEq)]
pub enum NodeType {
    Optical(String),
    Analyzer(AnalyzerType),
}
impl Default for NodeType {
    fn default() -> Self {
        Self::Optical(String::new())
    }
}
#[derive(Clone, PartialEq, Default)]
pub struct NodeElement {
    node_type: NodeType,
    id: Uuid,
    pos: Point2D<f64>,
    z_index: usize,
    ports: Ports,
}

impl NodeElement {
    #[must_use]
    pub const fn new(node_type: NodeType, id: Uuid, pos: Point2D<f64>, ports: Ports) -> Self {
        Self {
            node_type,
            pos,
            id,
            z_index: 0,
            ports,
        }
    }
    #[must_use]
    pub const fn input_ports(&self) -> &Vec<String> {
        self.ports.input_ports()
    }
    #[must_use]
    pub const fn output_ports(&self) -> &Vec<String> {
        self.ports.output_ports()
    }
    #[must_use]
    pub const fn z_index(&self) -> usize {
        self.z_index
    }
    #[must_use]
    pub fn pos(&self) -> Point2D<f64> {
        self.pos
    }
    #[must_use]
    pub fn name(&self) -> String {
        match &self.node_type {
            NodeType::Optical(name) => name.clone(),
            NodeType::Analyzer(analyzer_type) => format!("{analyzer_type}"),
        }
    }
    #[must_use]
    pub const fn id(&self) -> Uuid {
        self.id
    }
    pub fn shift_position(&mut self, shift: Point2D<f64>) {
        self.pos.x += shift.x;
        self.pos.y += shift.y;
    }
    pub fn rel_port_position(&self, port_type: &PortType, port_name: &str) -> Point2D<f64> {
        let (x_pos, port_list) = match port_type {
            PortType::Input => (0.0, self.input_ports()),
            PortType::Output => (NODE_WIDTH, self.output_ports()),
        };
        let port_index = port_list
            .iter()
            .position(|port| port == port_name)
            .unwrap_or(0);
        let y_pos = PORT_VER_PADDING + PORT_VER_SPACING * port_index as f64;
        Point2D::new(x_pos, y_pos)
    }
    pub fn abs_port_position(&self, port_type: &PortType, port_name: &str) -> Point2D<f64> {
        let rel_pos = self.rel_port_position(port_type, port_name);
        Point2D::new(
            self.pos.x + rel_pos.x,
            self.pos.y + rel_pos.y + HEADER_HEIGHT,
        )
    }
    pub fn node_body_height(&self) -> f64 {
        let max_vert_number_of_ports =
            usize_to_f64(self.output_ports().len().max(self.input_ports().len()));
        let necessary_body_height =
            2.0 * PORT_VER_PADDING + PORT_VER_SPACING * (max_vert_number_of_ports - 1.0);
        necessary_body_height.max(MIN_NODE_BODY_HEIGHT)
    }

    pub fn node_type(&self) -> &NodeType {
        &self.node_type
    }
}
