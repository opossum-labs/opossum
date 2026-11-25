use opossum_core::{
    core_optics::observable::ObservableDescriptor,
    opm_document::{
        OptimizationConfig,
        optimization::{ObservableAddress, ParameterAddress},
    },
};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use uuid::Uuid;

/// Display and parsing unit metadata for a tunable model parameter.
///
/// The solver and UI state always operate on SI base units (`m`, `rad`, `J`, etc.).
/// This metadata describes how the value is formatted and parsed in the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterUnit {
    /// Base unit symbol displayed in the UI (e.g. `"m"`, `"°"`, `"J"`, `"J/m²"`, `"1/m"`).
    pub base_unit: String,
    /// Whether engineering SI prefixes (`m`, `µ`, `k`, ...) should be applied.
    pub handle_prefix: bool,
    /// Multiplier converting from internal SI base unit to display base unit (e.g. `180.0 / PI` for `"°"`).
    pub display_scale: f64,
}

impl ParameterUnit {
    /// Creates a new [`ParameterUnit`] with `display_scale = 1.0`.
    #[must_use]
    pub fn new(base_unit: impl Into<String>, handle_prefix: bool) -> Self {
        Self {
            base_unit: base_unit.into(),
            handle_prefix,
            display_scale: 1.0,
        }
    }

    /// Length unit in meters (`m`) with SI prefix scaling (`mm`, `µm`, ...).
    #[must_use]
    pub fn length() -> Self {
        Self::new("m", true)
    }

    /// Planar angle displayed in degrees (`°`) without SI prefixes, backed by radians (`rad`).
    #[must_use]
    pub fn angle_deg() -> Self {
        Self {
            base_unit: "°".to_string(),
            handle_prefix: false,
            display_scale: 180.0 / PI,
        }
    }

    /// Optical energy in joules (`J`) with SI prefix scaling (`mJ`, `µJ`, ...).
    #[must_use]
    pub fn energy() -> Self {
        Self::new("J", true)
    }

    /// Optical fluence in `J/m²` with SI prefix scaling.
    #[must_use]
    pub fn fluence() -> Self {
        Self::new("J/m²", true)
    }

    /// Linear grating line density in `1/m` without SI prefixes.
    #[must_use]
    pub fn linear_density() -> Self {
        Self::new("1/m", false)
    }
}

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
    /// initial value (start value for optimization) in SI base units
    pub initial_value: f64,
    /// true if variable is read only
    pub is_read_only: bool,
    /// optional physical unit metadata for UI formatting and parsing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<ParameterUnit>,
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
