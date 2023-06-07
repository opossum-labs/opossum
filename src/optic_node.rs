use std::fmt::Debug;

use crate::optic_ports::OpticPorts;
/// An [`OpticNode`] is the basic struct representing an optical component.
pub struct OpticNode {
    name: String,
    node: Box<dyn Optical>,
    ports: OpticPorts
}

impl OpticNode {
    /// Creates a new [`OpticNode`]. The concrete type of the component must be given while using the `new` function.
    /// The node type ist a struct implementing the [`Optical`] trait. Since the size of the node type is not known at compile time it must be added as `Box<nodetype>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use opossum::optic_node::OpticNode;
    /// use opossum::nodes::NodeDummy;
    ///
    /// let node=OpticNode::new("My node", NodeDummy);
    /// ```
    pub fn new<T: Optical+ 'static>(name: &str, node_type: T) -> Self {
        let ports=node_type.ports();
        Self {
            name: name.into(),
            node: Box::new(node_type),
            ports
        }
    }
    /// Sets the name of this [`OpticNode`].
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }
    /// Returns a reference to the name of this [`OpticNode`].
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
    /// Returns a string representation of the [`OpticNode`] in `graphviz` format. This function is normally called by the top-level `to_dot`function within
    /// `OpticScenery`.
    pub fn to_dot(&self, node_index: &str) -> String {
        self.node.to_dot(node_index, &self.name, self.inverted())
    }
    /// Returns the concrete node type as string representation.
    pub fn node_type(&self) -> &str {
        self.node.node_type()
    }
    /// Mark the [`OpticNode`] as inverted.
    ///
    /// This means that the node is used in "reverse" direction. All output port become input parts and vice versa.
    pub fn set_inverted(&mut self, inverted: bool) {
        self.ports.set_inverted(inverted)
    }
    /// Returns if the [`OpticNode`] is used in reversed direction.
    pub fn inverted(&self) -> bool {
        self.ports.inverted()
    }
    /// Returns a reference to the [`OpticPorts`] of this [`OpticNode`].
    pub fn ports(&self) -> &OpticPorts {
        &self.ports
    }
}

impl Debug for OpticNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// This trait must be implemented by all concrete optical components.
pub trait Optical {
    /// Return the type of the optical component (lens, filter, ...). The default implementation returns "undefined".
    fn node_type(&self) -> &str {
        "undefined"
    }
    /// Return component type specific code for `graphviz` visualization.
    fn to_dot(&self, node_index: &str, name: &str, inverted: bool) -> String {
        let inv_string = if inverted { "(inv)" } else { "" };
        format!("  {} [label=\"{}{}\"]\n", node_index, name, inv_string)
    }
    fn ports(&self) -> OpticPorts {
        OpticPorts::default()
    }
}

#[cfg(test)]
mod test {
    use super::OpticNode;
    use crate::nodes::NodeDummy;
    #[test]
    fn new() {
        let node = OpticNode::new("Test", NodeDummy);
        assert_eq!(node.name, "Test");
        assert_eq!(node.inverted(), false);
    }
    #[test]
    fn set_name() {
        let mut node = OpticNode::new("Test", NodeDummy);
        node.set_name("Test2".into());
        assert_eq!(node.name, "Test2")
    }
    #[test]
    fn name() {
        let node = OpticNode::new("Test", NodeDummy);
        assert_eq!(node.name(), "Test")
    }
    #[test]
    fn set_inverted() {
        let mut node = OpticNode::new("Test", NodeDummy);
        node.set_inverted(true);
        assert_eq!(node.inverted(), true)
    }
    #[test]
    fn inverted() {
        let mut node = OpticNode::new("Test", NodeDummy);
        node.set_inverted(true);
        assert_eq!(node.inverted(), true)
    }
    #[test]
    fn to_dot() {
        let node = OpticNode::new("Test", NodeDummy);
        assert_eq!(node.to_dot("i0"), "  i0 [label=\"Test\"]\n".to_owned())
    }
    #[test]
    fn to_dot_inverted() {
        let mut node = OpticNode::new("Test", NodeDummy);
        node.set_inverted(true);
        assert_eq!(node.to_dot("i0"), "  i0 [label=\"Test(inv)\"]\n".to_owned())
    }
    #[test]
    fn node_type() {
        let node = OpticNode::new("Test", NodeDummy);
        assert_eq!(node.node_type(), "dummy");
    }
}
