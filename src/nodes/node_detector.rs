use crate::{optic_node::{Optical, Dottable}, optic_ports::OpticPorts};

pub struct NodeDetector;

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

impl Dottable for NodeDetector{
    fn node_color(&self) -> &str {
        "lemonchiffon"
      }
}

