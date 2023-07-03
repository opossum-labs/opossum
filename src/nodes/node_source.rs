use crate::{
    lightdata::LightData,
    optic_node::{Dottable, Optical},
    optic_ports::OpticPorts,
};

#[derive(Debug, Default)]
pub struct NodeSource {
    light_data: Option<LightData>,
}

impl NodeSource {
    pub fn new(light: LightData) -> Self {
        NodeSource {
            light_data: Some(light),
        }
    }

    pub fn light_data(&self) -> Option<&LightData> {
        self.light_data.as_ref()
    }

    pub fn set_light_data(&mut self, light_data: LightData) {
        self.light_data = Some(light_data);
    }
}
impl Optical for NodeSource {
    fn node_type(&self) -> &str {
        "light source"
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_output("out1").unwrap();
        ports
    }
}

impl Dottable for NodeSource {
    fn node_color(&self) -> &str {
        "slateblue"
    }
}
