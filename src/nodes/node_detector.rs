use std::fmt::Display;

use crate::{optic_node::{Optical, Dottable, LightResult}, optic_ports::OpticPorts};
use crate::lightdata::LightData;

#[derive(Debug, Default)]
pub struct NodeDetector {
  light_data: Option<LightData>
}

impl Optical for NodeDetector {
  fn node_type(&self) -> &str {
      "light sink: detector"
  }
  fn ports(&self) -> OpticPorts {
      let mut ports=OpticPorts::new();
      ports.add_input("in1").unwrap();
      ports
  }
  fn analyze(&mut self, incoming_data: LightResult, _analyzer_type: &crate::analyzer::AnalyzerType) -> LightResult {
    let data=incoming_data.into_iter().filter(|data| data.0=="in1").last();
    if let Some(data)=data {
      self.light_data=Some(data.1);
    }
    LightResult::default()
}
}

impl Dottable for NodeDetector{
    fn node_color(&self) -> &str {
        "lemonchiffon"
      }
}

