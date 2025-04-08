use dioxus::prelude::*;
use uuid::Uuid;

pub mod node_component;
pub use node_component::Node;

use crate::{EDGES, ZOOM};

use super::{
    node_drag_drop_container::drag_drop_container::ZoomShift, ports::ports_component::Ports,
    NodesStore,
};

#[derive(Clone, PartialEq, Default)]
pub struct NodeElement {
    x: f64,
    y: f64,
    z_index: usize,
    id: Uuid,
    name: String,
    is_active: bool,
    ports: Ports,
}

impl NodeElement {
    pub fn new(
        x: f64,
        y: f64,
        id: Uuid,
        name: String,
        is_active: bool,
        z_index: usize,
        ports: Ports,
    ) -> Self {
        NodeElement {
            x,
            y,
            id,
            name,
            is_active,
            z_index,
            ports,
        }
    }
    pub fn ports(&self) -> &Ports {
        &self.ports
    }
    pub fn input_ports(&self) -> &Vec<String> {
        self.ports.input_ports()
    }
    pub fn output_ports(&self) -> &Vec<String> {
        self.ports.output_ports()
    }
    pub fn z_index(&self) -> usize {
        self.z_index
    }
    pub fn set_z_index(&mut self, z_index: usize) {
        self.z_index = z_index;
    }
    pub fn set_inactive(&mut self) {
        self.is_active = false;
    }
    pub fn set_active(&mut self) {
        self.is_active = true;
    }
    pub fn is_active(&self) -> bool {
        self.is_active
    }
    pub fn set_x(&mut self, new_x: f64) {
        self.x = new_x;
    }
    pub fn set_y(&mut self, new_y: f64) {
        self.y = new_y;
    }
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }
    pub fn x(&self) -> f64 {
        self.x
    }
    pub fn y(&self) -> f64 {
        self.y
    }
    pub fn name(&self) -> String {
        self.name.clone()
    }
    pub fn id(&self) -> &Uuid {
        &self.id
    }
    pub fn set_id(&mut self, id: Uuid) {
        self.id = id;
    }

    pub fn drag_node(
        &mut self,
        mouse_x: f64,
        mouse_y: f64,
        offset_x: f64,
        offset_y: f64,
        elem_offset_x: f64,
        elem_offset_y: f64,
    ) {
        let size = NodesStore::size();

        let x_init = self.x();
        let y_init = self.y();

        let shift_x = x_init - size.x / 2. + elem_offset_x;
        let shift_y = y_init - size.y / 2. + elem_offset_y;

        self.zoom_shift(
            ZOOM.read().current(),
            (mouse_x - offset_x, mouse_y - offset_y),
            (shift_x, shift_y),
        );

        EDGES
            .write()
            .shift_if_connected(self.x() - x_init, self.y() - y_init, *self.id());
    }
}

impl ZoomShift for NodeElement {
    fn zoom_shift(&mut self, zoom_factor: f64, offset: (f64, f64), shift: (f64, f64)) {
        let new_x = offset.0 + (self.x() - shift.0) * zoom_factor;
        let new_y = offset.1 + (self.y() - shift.1) * zoom_factor;
        self.set_x(new_x);
        self.set_y(new_y);
    }
}
