use std::collections::HashMap;

use crate::{
    lightdata::LightData,
    optic_node::{Dottable, Optical, LightResult},
    optic_ports::OpticPorts, error::OpossumError,
};

type Result<T> = std::result::Result<T, OpossumError>;

/// This node represents a source of light. Hence it has only one output port (out1) and no input ports. Source nodes usually are the first nodes of an optic scenery.
#[derive(Debug, Default)]
pub struct NodeSource {
    light_data: Option<LightData>,
}

impl NodeSource {
    /// Creates a new [`NodeSource`].
    pub fn new(light: LightData) -> Self {
        NodeSource {
            light_data: Some(light),
        }
    }

    /// Returns the light data of this [`NodeSource`].
    pub fn light_data(&self) -> Option<&LightData> {
        self.light_data.as_ref()
    }

    /// Sets the light data of this [`NodeSource`]. The [`LightData`] provided here represents the input data of an `OpticScenery`.
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

    fn analyze(&mut self, _incoming_edges: LightResult, _analyzer_type: &crate::analyzer::AnalyzerType) -> Result<LightResult> {
        let data=self.light_data.clone();
        if data.is_some() {
            Ok(HashMap::from([("out1".into(), data)]))
        } else {
            Err(OpossumError::Analysis(format!("no input data available")))
        }
    }
}

impl Dottable for NodeSource {
    fn node_color(&self) -> &str {
        "slateblue"
    }
}
