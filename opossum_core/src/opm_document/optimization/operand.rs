use std::borrow::Cow;

use super::address::MetricSource;
use crate::{core_optics::observable::MetricDimension, utils::to_f64};
use serde::{Deserialize, Serialize};
use uom::si::{
    energy::joule,
    f64::{Energy, Length, RadiantExposure},
    length::meter,
    radiant_exposure::joule_per_square_meter,
};
use uuid::Uuid;

/// Default scale constants for optical observables in SI base units (meters).
pub const DEFAULT_SPOT_SCALE: f64 = 1.0e-6; // 1.0 micrometer
/// default scale constant for wavefront constraints
pub const DEFAULT_WAVEFRONT_SCALE: f64 = 1.0e-7; // 100.0 nanometers (~ lambda / 10)
/// defualt scale constant for geometric constraints
pub const DEFAULT_GEOMETRIC_SCALE: f64 = 1.0e-3; // 1.0 millimeter

/// Resolves a default normalization scale based on dimension and metric naming convention.
#[must_use]
pub fn default_scale_for_metric(dimension: MetricDimension, metric_name: &str) -> f64 {
    match dimension {
        MetricDimension::Length => {
            let lower = metric_name.to_lowercase();
            if lower.contains("wavefront") || lower.contains("ptv") || lower.contains("p-v") {
                DEFAULT_WAVEFRONT_SCALE
            } else if lower.contains("spot") || lower.contains("radius") || lower.contains("waist")
            {
                DEFAULT_SPOT_SCALE
            } else {
                DEFAULT_GEOMETRIC_SCALE
            }
        }
        MetricDimension::Dimensionless => 1.0,
        MetricDimension::Angle | MetricDimension::Energy => 1.0e-3, // 1.0 mrad / 1.0 mJ
        MetricDimension::Fluence => 1.0e4,                          // 1.0 J/cm^2 = 10,000 J/m^2
    }
}

/// Evaluation mode of a scalar optical metric operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OperandMode {
    /// Penalizes deviation from target symmetrically: (val - target)^2
    #[serde(rename = "equal")]
    Equal,
    /// Penalizes values exceeding the upper limit: max(0, val - limit)^2
    #[serde(rename = "upper_limit")]
    UpperLimit,
    /// Penalizes values falling below the lower limit: max(0, limit - val)^2
    #[serde(rename = "lower_limit")]
    LowerLimit,
}

impl OperandMode {
    /// Returns the operator symbol for table display.
    #[must_use]
    pub const fn symbol(&self) -> &'static str {
        match self {
            Self::Equal => "=",
            Self::UpperLimit => "≤",
            Self::LowerLimit => "≥",
        }
    }

    /// Cycles cyclically through Equal -> UpperLimit -> LowerLimit -> Equal.
    #[must_use]
    pub const fn cycle(&self) -> Self {
        match self {
            Self::Equal => Self::UpperLimit,
            Self::UpperLimit => Self::LowerLimit,
            Self::LowerLimit => Self::Equal,
        }
    }
}

/// Unified scalar merit operand targeting either an exact value or an inequality limit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricOperand {
    pub source: MetricSource,
    pub target: f64,
    pub mode: OperandMode,
    pub weight: f64,
    scale: f64,
}

impl MetricOperand {
    /// Creates a new [`MetricOperand`].
    pub fn new(
        source: impl Into<MetricSource>,
        target: f64,
        mode: OperandMode,
        weight: f64,
        scale: f64,
    ) -> Self {
        let scale = if scale.abs() < f64::EPSILON {
            1.0
        } else {
            scale
        };
        Self {
            source: source.into(),
            target,
            mode,
            weight,
            scale,
        }
    }

    /// Helper builder for setting target value operand
    pub fn target(source: impl Into<MetricSource>, target: f64, weight: f64, scale: f64) -> Self {
        Self::new(source, target, OperandMode::Equal, weight, scale)
    }

    /// Helper builder for setting an upper limit value operand
    pub fn upper_limit(
        source: impl Into<MetricSource>,
        limit: f64,
        weight: f64,
        scale: f64,
    ) -> Self {
        Self::new(source, limit, OperandMode::UpperLimit, weight, scale)
    }

    /// Helper builder for setting an upper limit value operand
    pub fn lower_limit(
        source: impl Into<MetricSource>,
        limit: f64,
        weight: f64,
        scale: f64,
    ) -> Self {
        Self::new(source, limit, OperandMode::LowerLimit, weight, scale)
    }

    /// Returns the name of this [`MetricOperand`].
    #[must_use]
    pub fn name(&self) -> String {
        format!("{} {} {}", self.source, self.mode.symbol(), self.target)
    }

    /// Returns the weight of this [`MetricOperand`].
    #[must_use]
    pub fn weight(&self) -> f64 {
        self.weight
    }

    /// Helper builder for spot radius operands
    pub fn for_spot_radius(
        source: impl Into<MetricSource>,
        target: Length,
        mode: OperandMode,
        weight: f64,
    ) -> Self {
        Self::new(
            source,
            target.get::<meter>(),
            mode,
            weight,
            DEFAULT_SPOT_SCALE,
        )
    }

    /// Helper builder for wavefront (in length units) operands
    pub fn for_wavefront(source: impl Into<MetricSource>, target: Length, weight: f64) -> Self {
        Self::new(
            source,
            target.get::<meter>(),
            OperandMode::Equal,
            weight,
            DEFAULT_WAVEFRONT_SCALE,
        )
    }

    /// Helper builder for wavefront (in lambda) operands
    pub fn for_wavefront_waves(
        source: impl Into<MetricSource>,
        target_waves: f64,
        weight: f64,
    ) -> Self {
        Self::new(source, target_waves, OperandMode::Equal, weight, 0.02)
    }

    /// Helper builder for length operands
    pub fn for_length(
        source: impl Into<MetricSource>,
        target: Length,
        mode: OperandMode,
        weight: f64,
    ) -> Self {
        Self::new(
            source,
            target.get::<meter>(),
            mode,
            weight,
            DEFAULT_GEOMETRIC_SCALE,
        )
    }

    /// Helper builder for energy operands
    pub fn for_energy(
        source: impl Into<MetricSource>,
        target: Energy,
        mode: OperandMode,
        weight: f64,
    ) -> Self {
        Self::new(source, target.get::<joule>(), mode, weight, 1.0e-3)
    }

    /// Helper builder for fluence operands
    pub fn for_fluence(
        source: impl Into<MetricSource>,
        target: RadiantExposure,
        mode: OperandMode,
        weight: f64,
    ) -> Self {
        Self::new(
            source,
            target.get::<joule_per_square_meter>(),
            mode,
            weight,
            1.0e4,
        )
    }
    /// Returns the scale of this [`MetricOperand`].
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// Returns the mode of this [`MetricOperand`].
    pub fn mode(&self) -> OperandMode {
        self.mode
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaxTotalLengthOperand {
    pub segments: Vec<super::address::ParameterAddress>,
    pub max_length: f64,
    pub weight: f64,
    scale: f64,
}

impl MaxTotalLengthOperand {
    /// Creates a new [`MaxTotalLengthOperand`].
    pub fn new(
        segments: Vec<super::address::ParameterAddress>,
        max_length: f64,
        weight: f64,
    ) -> Self {
        Self {
            segments,
            max_length,
            weight,
            scale: DEFAULT_GEOMETRIC_SCALE,
        }
    }
    /// Returns the name of this [`MaxTotalLengthOperand`].
    #[must_use]
    pub fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("MaxTotalLength")
    }
    /// Returns the weight of this [`MaxTotalLengthOperand`].
    #[must_use]
    pub fn weight(&self) -> f64 {
        self.weight
    }

    /// Helper builder for easy usage of uom values
    pub fn for_length(
        segments: Vec<super::address::ParameterAddress>,
        max_length: Length,
        weight: f64,
    ) -> Self {
        Self::new(segments, max_length.get::<meter>(), weight)
    }

    /// Returns the scale of this [`MaxTotalLengthOperand`].
    #[must_use]
    pub fn scale(&self) -> f64 {
        self.scale
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MinEdgeThicknessOperand {
    pub node_id: Uuid,
    pub min_thickness: f64,
    pub weight: f64,
    scale: f64,
}

impl MinEdgeThicknessOperand {
    /// Creates a new [`MinEdgeThicknessOperand`].
    #[must_use]
    pub fn new(node_id: Uuid, min_thickness: f64, weight: f64) -> Self {
        Self {
            node_id,
            min_thickness,
            weight,
            scale: DEFAULT_GEOMETRIC_SCALE,
        }
    }
    /// Returns the name of this [`MinEdgeThicknessOperand`].
    #[must_use]
    pub fn name(&self) -> String {
        format!("MinEdgeThickness({})", self.node_id)
    }
    /// Returns the weight of this [`MinEdgeThicknessOperand`].
    #[must_use]
    pub fn weight(&self) -> f64 {
        self.weight
    }
    #[must_use]
    pub fn for_thickness(node_id: Uuid, min_thickness: Length, weight: f64) -> Self {
        Self::new(node_id, min_thickness.get::<meter>(), weight)
    }

    /// Returns the scale of this [`MinEdgeThicknessOperand`].
    #[must_use]
    pub fn scale(&self) -> f64 {
        self.scale
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransmissionPenaltyOperand {
    pub address: super::address::ObservableAddress,
    pub expected_rays: f64,
    pub weight: f64,
}

impl TransmissionPenaltyOperand {
    /// Creates a new [`TransmissionPenaltyOperand`].
    pub fn new(
        address: super::address::ObservableAddress,
        expected_rays: usize,
        weight: f64,
    ) -> Self {
        Self {
            address,
            expected_rays: to_f64(expected_rays),
            weight,
        }
    }
    /// Returns the name of this [`TransmissionPenaltyOperand`].
    #[must_use]
    pub fn name(&self) -> String {
        format!("TransmissionPenalty({})", self.address)
    }
    /// Returns the weight of this [`TransmissionPenaltyOperand`].
    #[must_use]
    pub fn weight(&self) -> f64 {
        self.weight
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StandardOperand {
    Metric(MetricOperand),
    MaxTotalLength(MaxTotalLengthOperand),
    MinEdgeThickness(MinEdgeThicknessOperand),
    TransmissionPenalty(TransmissionPenaltyOperand),
}

impl StandardOperand {
    /// Returns the name of this [`StandardOperand`].
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::Metric(op) => op.name(),
            Self::MaxTotalLength(op) => op.name().into_owned(),
            Self::MinEdgeThickness(op) => op.name(),
            Self::TransmissionPenalty(op) => op.name(),
        }
    }

    /// Returns the weight of this [`StandardOperand`].
    #[must_use]
    pub fn weight(&self) -> f64 {
        match self {
            Self::Metric(op) => op.weight(),
            Self::MaxTotalLength(op) => op.weight(),
            Self::MinEdgeThickness(op) => op.weight(),
            Self::TransmissionPenalty(op) => op.weight(),
        }
    }
}

impl From<MetricOperand> for StandardOperand {
    fn from(op: MetricOperand) -> Self {
        Self::Metric(op)
    }
}
impl From<MaxTotalLengthOperand> for StandardOperand {
    fn from(op: MaxTotalLengthOperand) -> Self {
        Self::MaxTotalLength(op)
    }
}
impl From<MinEdgeThicknessOperand> for StandardOperand {
    fn from(op: MinEdgeThicknessOperand) -> Self {
        Self::MinEdgeThickness(op)
    }
}
impl From<TransmissionPenaltyOperand> for StandardOperand {
    fn from(op: TransmissionPenaltyOperand) -> Self {
        Self::TransmissionPenalty(op)
    }
}
