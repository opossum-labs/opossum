#[derive(Debug, Default)]
pub struct OpticNode {
    name: String,
}

impl OpticNode {
    /// Creates a new [`OpticNode`].
    pub fn new(name: String) -> Self {
        Self { name }
    }

    /// Sets the name of this [`OpticNode`].
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    /// Returns a reference to the name of this [`OpticNode`].
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}

#[cfg(test)]
mod test {
    use super::OpticNode;
    #[test]
    fn new() {
      let node = OpticNode::new("Test".into());
      assert_eq!(node.name, "Test".to_owned());
    }
    #[test]
    fn set_name() { 
        let mut node = OpticNode::new("Test".into());
        node.set_name("Test2".into());
        assert_eq!(node.name, "Test2".to_owned())
    }
    #[test]
    fn name() { 
        let node = OpticNode::new("Test".into());
        assert_eq!(node.name(), "Test".to_owned())
    }
}
