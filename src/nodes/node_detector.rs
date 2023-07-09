use crate::lightdata::LightData;
use crate::{
    error::OpossumError,
    optic_node::{Dottable, LightResult, Optical},
    optic_ports::OpticPorts,
};
use std::fmt::Debug;

type Result<T> = std::result::Result<T, OpossumError>;

#[derive(Default)]
/// This node rerpresents an universal detector. Any [`LightData`] coming in will be stored internally for later display / export. So far it only has one input (in1).
pub struct Detector {
    light_data: Option<LightData>,
}

impl Optical for Detector {
    fn node_type(&self) -> &str {
        "light sink: detector"
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_input("in1").unwrap();
        ports
    }
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        _analyzer_type: &crate::analyzer::AnalyzerType,
    ) -> Result<LightResult> {
        let data = incoming_data
            .into_iter()
            .filter(|data| data.0 == "in1")
            .last();
        if let Some(data) = data {
            self.light_data = data.1;
        }
        Ok(LightResult::default())
    }
}

impl Debug for Detector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.light_data {
            Some(data) => write!(f,"{}",data),
            None => write!(f, "no data"),
        }
    }
}
impl Dottable for Detector {
    fn node_color(&self) -> &str {
        "lemonchiffon"
    }
}
