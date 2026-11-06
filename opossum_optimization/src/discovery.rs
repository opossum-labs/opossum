use opossum_core::core_optics::observable::ObservableDescriptor;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::address::{ObservableAddress, ParameterAddress};

/// Metadata for an analyzer registered in the optical document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredAnalyzer {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
}

/// Metadata for a tunable parameter discovered in the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredVariable {
    pub address: ParameterAddress,
    pub group_id: Option<Uuid>,
    pub group_name: String,
    pub node_name: String,
    pub display_name: String,
    pub initial_value: f64,
    pub is_read_only: bool,
}

/// Metadata for an observable physical metric produced by an analyzer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredObservable {
    pub address: ObservableAddress,
    pub analyzer_name: String,
    pub analyzer_kind: String,
    pub node_name: String,
    pub descriptor: ObservableDescriptor,
}

/// Node eligible for edge thickness geometric constraint checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredEdgeThicknessNode {
    pub node_id: Uuid,
    pub node_name: String,
    pub node_type: String,
}

/// Complete discoverable optimization schema extracted from an optical document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ModelOptimizationSchema {
    pub controllable_variables: Vec<DiscoveredVariable>,
    pub observable_metrics: Vec<DiscoveredObservable>,
    pub edge_thickness_nodes: Vec<DiscoveredEdgeThicknessNode>,
    pub analyzers: Vec<DiscoveredAnalyzer>,
}