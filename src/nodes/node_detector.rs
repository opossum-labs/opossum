use std::fmt::Display;

use crate::{optic_node::{Optical, Dottable}, optic_ports::OpticPorts};
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
  
}

impl Display for NodeDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(f, "Detector: ").unwrap();
      if self.light_data.is_none() {
        write!(f,"no data available")
      } else {
        write!(f, "{:?}", self.light_data)
      }
    }
}

impl Dottable for NodeDetector{
    fn node_color(&self) -> &str {
        "lemonchiffon"
      }
}

