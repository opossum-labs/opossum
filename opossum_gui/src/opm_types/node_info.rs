use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct NodeInfo {
    uuid: Uuid,
    name: String,
    node_type: String,
}

impl NodeInfo {
    #[must_use]
    pub const fn new(uuid: Uuid, name: String, node_type: String) -> Self {
        Self {
            uuid,
            name,
            node_type,
        }
    }
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.uuid
    }
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn node_type(&self) -> &str {
        &self.node_type
    }
}
