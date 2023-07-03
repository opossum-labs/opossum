use crate::{optic_node::{Optical, Dottable}, optic_ports::OpticPorts};

pub struct NodeSource;

impl Optical for NodeSource {
  fn node_type(&self) -> &str {
      "light source"
  }
  fn ports(&self) -> OpticPorts {
      let mut ports=OpticPorts::new();
      ports.add_output("out1").unwrap();
      ports
  }
  
}

impl Dottable for NodeSource{
  fn node_color(&self) -> &str {
    "slateblue"
  }
}
