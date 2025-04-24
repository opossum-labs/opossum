use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct NodeAttr {
    node_type: String,
    name: String,
}

#[derive(Clone, PartialEq)]
pub enum PortType {
    Input,
    Output,
}
impl NodeAttr {
    /// Returns the name property of this node.
    ///
    /// # Errors
    ///
    /// This function will return an error if the property `name` and the property `node_type` does not exist.
    #[must_use]
    pub fn name(&self) -> String {
        self.name.clone()
    }
}
