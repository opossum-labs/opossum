use super::address::{ObservableAddress, ParameterAddress};
use opossum_core::core_optics::observable::ObservableDescriptor;
use serde::{Deserialize, Serialize};

/// Discovered variable parameter candidate available for optimization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredVariable {
    pub address: ParameterAddress,
    pub node_name: String,
    pub display_name: String,
    pub initial_value: f64,
    pub is_read_only: bool,
}

/// Discovered observable metric available as an optimization operand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredObservable {
    pub address: ObservableAddress,
    pub node_name: String,
    pub descriptor: ObservableDescriptor,
}

/// Complete inventory of all controllable parameters and observable outputs of an OpmDocument.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ModelOptimizationSchema {
    pub controllable_variables: Vec<DiscoveredVariable>,
    pub observable_metrics: Vec<DiscoveredObservable>,
}
