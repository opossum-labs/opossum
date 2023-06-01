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
        let mut dot_string="digraph {\n".to_owned();
        dot_string.push_str(&format!("  label=\"{}\"\n", self.description));
        dot_string+="}";
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
