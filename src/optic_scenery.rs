use crate::optic_node::OpticNode;
use petgraph::prelude::{DiGraph, NodeIndex};

/// Opticscenery represents the overall optical model and additional metatdata. All optical elements (OpticNodes) have to be added to this
/// structure in order to be considered for an analysis.
#[derive(Default, Debug)]
pub struct OpticScenery {
    g: DiGraph<OpticNode, ()>,
    description: String,
}

impl OpticScenery {
    /// Creates a new (default) [`OpticScenery`].
    pub fn new() -> Self {
        Self::default()
    }
    /// Add a given [`OpticNode`] to the graph of this [`OpticScenery`].
    ///
    /// This command just adds an [`OpticNode`] but does not connect it to existing nodes in the graph. The given node is
    /// consumed (owned) by the [`OpticScenery`].
    pub fn add_node(&mut self, node: OpticNode) -> NodeIndex {
        self.g.add_node(node)
    }
    /// Export the optic graph into the `dot` format to be used in combination with the [`graphviz`](https://graphviz.org/) software.
    pub fn to_dot(&self) -> String {
        let mut dot_string = "digraph {\n".to_owned();
        dot_string.push_str(&format!("  label=\"{}\"\n", self.description));
        for node in self.g.node_weights() {
            dot_string += &node.to_dot();
        }
        dot_string += "}";
        dot_string
    }
    /// Analyze this [`OpticScenery`] using a given OpticAnalyzer.
    pub fn analyze(&self) {
        todo!();
    }
    /// Sets the description of this [`OpticScenery`].
    pub fn set_description(&mut self, description: String) {
        self.description = description;
    }
    /// Returns a reference to the description of this [`OpticScenery`].
    pub fn description(&self) -> &str {
        self.description.as_ref()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn new() {
        let scenery = OpticScenery::new();
        assert_eq!(scenery.description, "".to_owned());
        assert_eq!(scenery.g.edge_count(), 0);
        assert_eq!(scenery.g.node_count(), 0);
    }
    #[test]
    fn add_node() {
        let mut scenery = OpticScenery::new();
        scenery.add_node(OpticNode::new("Test".into(), Box::new(NodeDummy)));
        assert_eq!(scenery.g.node_count(), 1);
    }
    #[test]
    fn to_dot_empty() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("Test".into());
        assert_eq!(scenery.to_dot(), "digraph {\n  label=\"Test\"\n}");
    }
    #[test]
    fn to_dot_with_node() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("SceneryTest".into());
        scenery.add_node(OpticNode::new("Test".into(), Box::new(NodeDummy)));
        assert_eq!(
            scenery.to_dot(),
            "digraph {\n  label=\"SceneryTest\"\n  Test\n}"
        );
    }
    #[test]
    fn set_description() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("Test".into());
        assert_eq!(scenery.description, "Test".to_owned())
    }
    #[test]
    fn description() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("Test".into());
        assert_eq!(scenery.description(), "Test".to_owned())
    }
}
