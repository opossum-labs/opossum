use crate::{optic_node::Optical, optic_ports::OpticPorts};

pub struct NodeBeamSplitter;

impl Optical for NodeBeamSplitter {
  fn node_type(&self) -> &str {
      "ideal beam splitter"
  }
  fn ports(&self) -> OpticPorts {
      let mut ports=OpticPorts::new();
      ports.add_input("input1").unwrap();
      ports.add_input("input2").unwrap();
      ports.add_output("out1_trans1_refl2").unwrap();
      ports.add_output("out2_trans2_refl1").unwrap();
      ports
  }
  
}

