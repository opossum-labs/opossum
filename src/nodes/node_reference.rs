use crate::optic_node::{OpticNode, Optical};

/// A virtual component referring to another existing component. This node type is necessary in order to model resonators (loops) or double-pass systems.
pub struct NodeReference<'a> {
    reference: &'a OpticNode,
}

impl<'a> NodeReference<'a> {
    pub fn new(node: &'a OpticNode) -> Self {
        Self { reference: node }
    }
}

impl<'a> Optical for NodeReference<'a> {
    /// Returns "dummy" as node type.
    fn node_type(&self) -> &str {
        "reference"
    }
}
