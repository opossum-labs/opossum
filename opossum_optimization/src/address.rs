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
    /// A distance starting from an output port of a node.
    ConnectionDistance { src_id: Uuid, src_port: String },
}

/// Unique address identifying an observable output metric (operand source).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObservableAddress {
    /// Analyzer producing the evaluation.
    pub analyzer_id: Uuid,
    /// Detector node producing the metric.
    pub node_id: Uuid,
    /// Key of the observable metric (e.g. "Energy", "rms beam radius").
    pub metric_name: String,
}

/// Unified source for quantities evaluated by optimization operands.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricSource {
    /// A physics metric recorded by a detector during analysis.
    Observable(ObservableAddress),
    /// A geometric or model property (e.g. thickness, radius, spacing).
    Parameter(ParameterAddress),
}

impl From<ObservableAddress> for MetricSource {
    fn from(address: ObservableAddress) -> Self {
        Self::Observable(address)
    }
}

impl From<ParameterAddress> for MetricSource {
    fn from(address: ParameterAddress) -> Self {
        Self::Parameter(address)
    }
}
