use serde::{Deserialize, Serialize};

/// Identifies the physical dimension of an observable scalar metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricDimension {
    Dimensionless,
    Length,
    Angle,
    Energy,
    Fluence,
}

/// Metadata describing a measurable metric provided by a detector node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservableDescriptor {
    /// Internal identifier, matching the key in `NodeReport` properties (e.g. `Energy`, `rms_radius`).
    pub name: String,
    /// Human readable label for UI.
    pub display_name: String,
    /// Physical dimension in SI base units.
    pub dimension: MetricDimension,
    /// Default optimization target if applicable.
    pub default_target: Option<f64>,
}

impl ObservableDescriptor {
    /// Creates a new metric descriptor.
    pub fn new(
        name: impl Into<String>,
        display_name: impl Into<String>,
        dimension: MetricDimension,
    ) -> Self {
        Self {
            name: name.into(),
            display_name: display_name.into(),
            dimension,
            default_target: None,
        }
    }

    /// Sets an optional default target.
    #[must_use]
    pub const fn with_default_target(mut self, target: f64) -> Self {
        self.default_target = Some(target);
        self
    }
}
