use std::fmt::Debug;
/// An [`OpticNode`] is the basic struct representing an optical component.
pub struct OpticNode {
    name: String,
    node: Box<dyn Optical>,
    inverted: bool
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
    /// let node=OpticNode::new("My node", Box::new(NodeDummy));
    /// ```
    pub fn new(name: &str, node: Box<dyn Optical>) -> Self {
        Self { name: name.into(), node: node, inverted: false}
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
    pub fn to_dot(&self) -> String {
        let is_inverted= if self.inverted==true {" (inv)"} else {""};
        format!("[label=\"{}{}\"]\n", self.name, is_inverted)
    }
    /// Returns the concrete node type as string representation.
    pub fn node_type(&self) -> &str {
        self.node.node_type()
    }
    /// Mark the [`OpticNode`] as inverted.
    /// 
    /// This means that the node is used in "reverse" direction. All output port become input parts and vice versa.
    pub fn set_inverted(&mut self, inverted: bool) {
        self.inverted = inverted;
    }
    /// Returns if the [`OpticNode`] is used in reversed direction.
    pub fn inverted(&self) -> bool {
        self.inverted
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
    fn to_dot(&self) -> &str {
        ""
    }
}

#[cfg(test)]
mod test {
    use super::OpticNode;
    use crate::nodes::NodeDummy;
    #[test]
    fn new() {
        let node = OpticNode::new("Test", Box::new(NodeDummy));
        assert_eq!(node.name, "Test");
        assert_eq!(node.inverted, false);
    }
    #[test]
    fn set_name() {
        let mut node = OpticNode::new("Test", Box::new(NodeDummy));
        node.set_name("Test2".into());
        assert_eq!(node.name, "Test2")
    }
    #[test]
    fn name() {
        let node = OpticNode::new("Test", Box::new(NodeDummy));
        assert_eq!(node.name(), "Test")
    }
    #[test]
    fn set_inverted() {
        let mut node = OpticNode::new("Test", Box::new(NodeDummy));
        node.set_inverted(true);
        assert_eq!(node.inverted, true)
    }
    #[test]
    fn inverted() {
        let mut node = OpticNode::new("Test", Box::new(NodeDummy));
        node.set_inverted(true);
        assert_eq!(node.inverted(), true)
    }
    #[test]
    fn to_dot() {
        let node = OpticNode::new("Test", Box::new(NodeDummy));
        assert_eq!(node.to_dot(), "[label=\"Test\"]\n".to_owned())
    }
    #[test]
    fn to_dot_inverted() {
        let mut node = OpticNode::new("Test", Box::new(NodeDummy));
        node.set_inverted(true);
        assert_eq!(node.to_dot(), "[label=\"Test (inv)\"]\n".to_owned())
    }
    #[test]
    fn node_type() {
        let node = OpticNode::new("Test", Box::new(NodeDummy));
        assert_eq!(node.node_type(), "dummy");
    }
}
