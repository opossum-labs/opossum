use dioxus::html::geometry::euclid::default::Point2D;
use opossum_backend::PortType;
use uuid::Uuid;
pub mod node_component;
pub use node_component::Node;

use super::ports::ports_component::Ports;

#[derive(Clone, PartialEq, Default)]
pub struct NodeElement {
    name: String,
    id: Uuid,
    pos: Point2D<f64>,
    z_index: usize,
    ports: Ports,
}

impl NodeElement {
    #[must_use]
    pub const fn new(
        name: String,
        id: Uuid,
        pos: Point2D<f64>,
        z_index: usize,
        ports: Ports,
    ) -> Self {
        Self {
            pos,
            id,
            name,
            z_index,
            ports,
        }
    }
    #[must_use]
    pub const fn ports(&self) -> &Ports {
        &self.ports
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
    pub const fn set_z_index(&mut self, z_index: usize) {
        self.z_index = z_index;
    }
    pub const fn set_pos(&mut self, pos: Point2D<f64>) {
        self.pos = pos;
    }
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }
    #[must_use]
    pub fn pos(&self) -> Point2D<f64> {
        self.pos
    }
    #[must_use]
    pub fn name(&self) -> String {
        self.name.clone()
    }
    #[must_use]
    pub const fn id(&self) -> &Uuid {
        &self.id
    }
    pub fn shift_position(&mut self, shift: Point2D<f64>) {
        self.pos.x += shift.x;
        self.pos.y += shift.y;
    }
    pub fn port_position(self, port_type: PortType, port_name: &str) -> Point2D<f64> {
        Point2D::new(0.0,0.0)
    }
}
