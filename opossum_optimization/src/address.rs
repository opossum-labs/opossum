use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique address identifying a tunable variable in the optical model.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParameterAddress {
    /// A scalar property located on a node (e.g. radius of curvature, thickness, tilt).
    NodeProperty {
        node_id: Uuid,
        property_name: String,
    },
    /// A distance between two connected nodes inside a NodeGroup.
    ConnectionDistance {
        parent_group_id: Uuid,
        src_id: Uuid,
        src_port: String,
    },
}

/// Unique address identifying an observable output metric (operand source).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObservableAddress {
    /// Analyzer producing the evaluation.
    pub analyzer_id: Uuid,
    /// Detector node producing the metric.
    pub node_id: Uuid,
    /// Key of the observable metric (e.g. "Energy").
    pub metric_name: String,
}