use crate::optic_node::{Optical,Dottable};
use crate::optic_ports::OpticPorts;

#[derive(Debug)]
/// A fake / dummy component without any functions. It is mainly used for development and debugging purposes.
pub struct NodeDummy;

impl Optical for NodeDummy {
    /// Returns "dummy" as node type.
    fn node_type(&self) -> &str {
        "dummy"
    }
    fn ports(&self) -> OpticPorts {
        let mut ports=OpticPorts::new();
        ports.add_input("front").unwrap();
        ports.add_output("rear").unwrap();
        ports
    }
}

impl Dottable for NodeDummy{}