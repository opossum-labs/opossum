use crate::{optic_node::Optical, optic_ports::OpticPorts};

pub struct NodeBeamSplitter;

impl Optical for NodeBeamSplitter {
  fn node_type(&self) -> &str {
      "ideal beam splitter"
  }
  fn ports(&self) -> OpticPorts {
      let mut ports=OpticPorts::new();
      ports.add_input("input").unwrap();
      ports.add_output("transmitted").unwrap();
      ports.add_output("reflected").unwrap();
      ports
  }
}

