use crate::optic_node::Optical;

/// A fake / dummy component without any functions. It is mainly used for development and debugging purposes.
pub struct NodeDummy;

impl Optical for NodeDummy {
    /// Returns "dummy" as node type.
    fn node_type(&self) -> &str {
        "dummy"
    }
}