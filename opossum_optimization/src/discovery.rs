use super::address::{ObservableAddress, ParameterAddress};
use opossum_core::core_optics::observable::ObservableDescriptor;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Discovered variable parameter candidate available for optimization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredVariable {
    pub address: ParameterAddress,
    pub node_name: String,
    pub display_name: String,
    pub initial_value: f64,
    pub is_read_only: bool,
}

/// Discovered observable metric available as an optimization operand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredObservable {
    pub address: ObservableAddress,
    pub node_name: String,
    pub descriptor: ObservableDescriptor,
}

/// Optical element candidate supporting physical edge thickness constraints (e.g. lenses).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiscoveredEdgeThicknessNode {
    pub node_id: Uuid,
    pub node_name: String,
    pub node_type: String,
}

/// Complete inventory of all controllable parameters and observable outputs of an `OpmDocument`.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelOptimizationSchema {
    pub controllable_variables: Vec<DiscoveredVariable>,
    pub observable_metrics: Vec<DiscoveredObservable>,
    pub edge_thickness_nodes: Vec<DiscoveredEdgeThicknessNode>,
}
