use std::fmt::Debug;
/// An [`OpticNode`] is the basic struct representing an optical component.
pub struct OpticNode {
    name: String,
    node: Box<dyn Optical>,
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
    /// let node=OpticNode::new("My node".into(), Box::new(NodeDummy));
    /// ```
    pub fn new(name: String, node: Box<dyn Optical>) -> Self {
        Self { name, node }
    }
    /// Sets the name of this [`OpticNode`].
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }
    /// Returns a reference to the name of this [`OpticNode`].
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
    /// Returns a string representation of the [`OpticNode`] in `graphviz` format.
    pub fn to_dot(&self) -> String {
        format!("  \"{}\"\n", self.name)
    }
}

impl Debug for OpticNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
pub trait Optical {}

#[cfg(test)]
mod test {
    use super::OpticNode;
    use crate::nodes::NodeDummy;
    #[test]
    fn new() {
        let node = OpticNode::new("Test".into(), Box::new(NodeDummy));
        assert_eq!(node.name, "Test".to_owned());
    }
    #[test]
    fn set_name() {
        let mut node = OpticNode::new("Test".into(), Box::new(NodeDummy));
        node.set_name("Test2".into());
        assert_eq!(node.name, "Test2".to_owned())
    }
    #[test]
    fn name() {
        let node = OpticNode::new("Test".into(), Box::new(NodeDummy));
        assert_eq!(node.name(), "Test".to_owned())
    }
    #[test]
    fn to_dot() {
        let node = OpticNode::new("Test".into(), Box::new(NodeDummy));
        assert_eq!(node.to_dot(), "  \"Test\"\n".to_owned())
    }
}
