use opossum_core::{
    core_optics::observable::ObservableDescriptor,
    opm_document::{
        OptimizationConfig,
        optimization::{ObservableAddress, ParameterAddress},
    },
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Metadata for an analyzer registered in the optical document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredAnalyzer {
    /// id of the discovered analyzer
    pub id: Uuid,
    /// name of the discovered analyzer
    pub name: String,
    /// type of the discovered analyzer
    pub kind: String,
}

/// Metadata for a tunable parameter discovered in the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredVariable {
    /// unique address
    pub address: ParameterAddress,
    /// id of the `NodeGroup` this variable belongs to
    pub group_id: Option<Uuid>,
    /// name of the `NodeGroup` this variable belongs to
    pub group_name: String,
    /// node name
    pub node_name: String,
    /// displayed node name
    pub display_name: String,
    /// initial value (start value for optimization)
    pub initial_value: f64,
    /// true if variable is read only
    pub is_read_only: bool,
}

/// Metadata for an observable physical metric produced by an analyzer.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DiscoveredObservable {
    /// unique address
    pub address: ObservableAddress,
    /// id of the `NodeGroup` this observable belongs to
    pub group_id: Option<uuid::Uuid>,
    /// name of the `NodeGroup` this variable belongs to
    pub group_name: String,
    /// name of the analyzer for this observable
    pub analyzer_name: String,
    /// type of the analyzer for this observable
    pub analyzer_kind: String,
    /// name the node this observalbe belongs to
    pub node_name: String,
    /// description of the observable
    pub descriptor: ObservableDescriptor,
}

/// Node eligible for edge thickness geometric constraint checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredEdgeThicknessNode {
    /// id of the node
    pub node_id: Uuid,
    /// name of the node
    pub node_name: String,
    /// node type
    pub node_type: String,
}

/// Complete discoverable optimization schema extracted from an optical document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ModelOptimizationSchema {
    /// list of discovered controllable variants
    pub controllable_variables: Vec<DiscoveredVariable>,
    /// list of discovered metrics
    pub observable_metrics: Vec<DiscoveredObservable>,
    /// list of discovered nodes with edge thickness parameters
    pub edge_thickness_nodes: Vec<DiscoveredEdgeThicknessNode>,
    /// list of discovered analyzers
    pub analyzers: Vec<DiscoveredAnalyzer>,
    /// optimization configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_config: Option<OptimizationConfig>,
}
