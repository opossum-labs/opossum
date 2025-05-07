use dioxus::html::geometry::euclid::default::Point2D;
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
    pub const fn set_id(&mut self, id: Uuid) {
        self.id = id;
    }

    pub fn shift_position(&mut self, shift: Point2D<f64>) {
        self.pos.x += shift.x;
        self.pos.y += shift.y;
    }
    // pub fn drag_node(
    //     &mut self,
    //     mouse_x: f64,
    //     mouse_y: f64,
    //     offset_x: f64,
    //     offset_y: f64,
    //     elem_offset_x: f64,
    //     elem_offset_y: f64,
    // ) {
    //     let size = NodesStore::size();

    //     let x_init = self.x();
    //     let y_init = self.y();

    //     let shift_x = x_init - size.x / 2. + elem_offset_x;
    //     let shift_y = y_init - size.y / 2. + elem_offset_y;

    //     self.zoom_shift(
    //         ZOOM.read().current(),
    //         (mouse_x - offset_x, mouse_y - offset_y),
    //         (shift_x, shift_y),
    //     );

    //     EDGES
    //         .write()
    //         .shift_if_connected(self.x() - x_init, self.y() - y_init, *self.id());
    // }
}
