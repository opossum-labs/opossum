use super::address::MetricSource;
use crate::{core_optics::observable::MetricDimension, utils::to_f64};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use uom::si::{
    energy::joule,
    f64::{Energy, Length, RadiantExposure},
    length::meter,
    radiant_exposure::joule_per_square_meter,
};
use uuid::Uuid;
/// Default normalization scale for beam spot / radius metrics in SI base units (`1.0e-6` m = 1.0 µm).
pub const DEFAULT_SPOT_SCALE: f64 = 1.0e-6; // 1.0 micrometer
/// Default normalization scale for wavefront error metrics in SI base units (`1.0e-7` m = 100.0 nm).
pub const DEFAULT_WAVEFRONT_SCALE: f64 = 1.0e-7; // 100.0 nanometers (~ lambda / 10)
/// Default normalization scale for macroscopic geometric constraints in SI base units (`1.0e-3` m = 1.0 mm).
pub const DEFAULT_GEOMETRIC_SCALE: f64 = 1.0e-3; // 1.0 millimeter
/// Resolves a default normalization scale based on physical [`MetricDimension`] and metric naming convention.
///
/// For [`MetricDimension::Length`], names containing `"wavefront"`, `"ptv"`, or `"p-v"` use
/// [`DEFAULT_WAVEFRONT_SCALE`], names containing `"spot"`, `"radius"`, or `"waist"` use
/// [`DEFAULT_SPOT_SCALE`], and all other length metrics fall back to [`DEFAULT_GEOMETRIC_SCALE`].
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
    /// Penalizes deviation from `target` symmetrically: $((v - \text{target}) / \text{scale})^2$.
    #[serde(rename = "equal")]
    Equal,
    /// Penalizes values exceeding the upper limit: $(\max(0, v - \text{limit}) / \text{scale})^2$.
    #[serde(rename = "upper_limit")]
    UpperLimit,
    /// Penalizes values falling below the lower limit: $(\max(0, \text{limit} - v) / \text{scale})^2$.
    #[serde(rename = "lower_limit")]
    LowerLimit,
}
impl OperandMode {
    /// Returns the mathematical comparison symbol (`"="`, `"≤"`, or `"≥"`) for UI table display.
    #[must_use]
    pub const fn symbol(&self) -> &'static str {
        match self {
            Self::Equal => "=",
            Self::UpperLimit => "≤",
            Self::LowerLimit => "≥",
        }
    }
    /// Cycles through `Equal` $\rightarrow$ `UpperLimit` $\rightarrow$ `LowerLimit` $\rightarrow$ `Equal`.
    #[must_use]
    pub const fn cycle(&self) -> Self {
        match self {
            Self::Equal => Self::UpperLimit,
            Self::UpperLimit => Self::LowerLimit,
            Self::LowerLimit => Self::Equal,
        }
    }
}
/// Unified scalar merit operand targeting either an exact value or a one-sided inequality limit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricOperand {
    /// Observable or parameter address providing the evaluated scalar value.
    pub source: MetricSource,
    /// Target value or inequality threshold in SI base units.
    pub target: f64,
    /// Comparison mode determining how residuals are computed.
    pub mode: OperandMode,
    /// Relative weighting factor applied to the squared normalized residual.
    pub weight: f64,
    scale: f64,
}
impl MetricOperand {
    /// Creates a new [`MetricOperand`].
    ///
    /// If `scale` is closer to zero than `f64::EPSILON`, it is sanitized to `1.0` to prevent division by zero.
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
    /// Helper builder for creating an equality target operand ([`OperandMode::Equal`]).
    pub fn target(source: impl Into<MetricSource>, target: f64, weight: f64, scale: f64) -> Self {
        Self::new(source, target, OperandMode::Equal, weight, scale)
    }
    /// Helper builder for creating an upper-limit inequality operand ([`OperandMode::UpperLimit`]).
    pub fn upper_limit(
        source: impl Into<MetricSource>,
        limit: f64,
        weight: f64,
        scale: f64,
    ) -> Self {
        Self::new(source, limit, OperandMode::UpperLimit, weight, scale)
    }
    /// Helper builder for creating a lower-limit inequality operand ([`OperandMode::LowerLimit`]).
    pub fn lower_limit(
        source: impl Into<MetricSource>,
        limit: f64,
        weight: f64,
        scale: f64,
    ) -> Self {
        Self::new(source, limit, OperandMode::LowerLimit, weight, scale)
    }
    /// Returns a human-readable summary label of this [`MetricOperand`].
    #[must_use]
    pub fn name(&self) -> String {
        format!("{} {} {}", self.source, self.mode.symbol(), self.target)
    }
    /// Returns the weight of this [`MetricOperand`].
    #[must_use]
    pub const fn weight(&self) -> f64 {
        self.weight
    }
    /// Helper builder for beam spot radius operands using [`DEFAULT_SPOT_SCALE`].
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
    /// Helper builder for wavefront error equality operands specified as a physical [`Length`]
    /// using [`DEFAULT_WAVEFRONT_SCALE`].
    pub fn for_wavefront(source: impl Into<MetricSource>, target: Length, weight: f64) -> Self {
        Self::new(
            source,
            target.get::<meter>(),
            OperandMode::Equal,
            weight,
            DEFAULT_WAVEFRONT_SCALE,
        )
    }
    /// Helper builder for wavefront error equality operands expressed in units of wavelength ($\lambda$)
    /// with a default normalization scale of `0.02` waves.
    pub fn for_wavefront_waves(
        source: impl Into<MetricSource>,
        target_waves: f64,
        weight: f64,
    ) -> Self {
        Self::new(source, target_waves, OperandMode::Equal, weight, 0.02)
    }
    /// Helper builder for macroscopic [`Length`] operands using [`DEFAULT_GEOMETRIC_SCALE`].
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
    /// Helper builder for optical [`Energy`] operands with a normalization scale of `1.0e-3` J (1 mJ).
    pub fn for_energy(
        source: impl Into<MetricSource>,
        target: Energy,
        mode: OperandMode,
        weight: f64,
    ) -> Self {
        Self::new(source, target.get::<joule>(), mode, weight, 1.0e-3)
    }
    /// Helper builder for fluence ([`RadiantExposure`]) operands with a normalization scale of `1.0e4` J/m² (1 J/cm²).
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
    /// Returns the normalization scale of this [`MetricOperand`].
    #[must_use]
    pub const fn scale(&self) -> f64 {
        self.scale
    }
    /// Returns the evaluation [`OperandMode`] of this [`MetricOperand`].
    #[must_use]
    pub const fn mode(&self) -> OperandMode {
        self.mode
    }
}
/// Inequality penalty operand constraining the sum of specified length parameters to not exceed `max_length`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaxTotalLengthOperand {
    /// Addresses of the distance or thickness parameters summed together.
    pub segments: Vec<super::address::ParameterAddress>,
    /// Maximum allowed total length in meters.
    pub max_length: f64,
    /// Relative weighting factor applied when the sum exceeds `max_length`.
    pub weight: f64,
    scale: f64,
}
impl MaxTotalLengthOperand {
    /// Creates a new [`MaxTotalLengthOperand`] from a raw `max_length` in meters.
    #[must_use]
    pub const fn new(
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
    /// Returns the display name of this [`MaxTotalLengthOperand`].
    #[must_use]
    pub const fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("MaxTotalLength")
    }
    /// Returns the weight of this [`MaxTotalLengthOperand`].
    #[must_use]
    pub const fn weight(&self) -> f64 {
        self.weight
    }
    /// Helper builder accepting a typed [`Length`] quantity for `max_length`.
    #[must_use]
    pub fn for_length(
        segments: Vec<super::address::ParameterAddress>,
        max_length: Length,
        weight: f64,
    ) -> Self {
        Self::new(segments, max_length.get::<meter>(), weight)
    }
    /// Returns the normalization scale of this [`MaxTotalLengthOperand`].
    #[must_use]
    pub const fn scale(&self) -> f64 {
        self.scale
    }
}
/// Geometric manufacturability operand enforcing a minimum edge thickness on a lens node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MinEdgeThicknessOperand {
    /// Unique ID of the lens node whose edge thickness is constrained.
    pub node_id: Uuid,
    /// Minimum required edge thickness in meters.
    pub min_thickness: f64,
    /// Relative weighting factor applied when the edge thickness falls below `min_thickness`.
    pub weight: f64,
    scale: f64,
}
impl MinEdgeThicknessOperand {
    /// Creates a new [`MinEdgeThicknessOperand`] from a raw `min_thickness` in meters.
    #[must_use]
    pub const fn new(node_id: Uuid, min_thickness: f64, weight: f64) -> Self {
        Self {
            node_id,
            min_thickness,
            weight,
            scale: DEFAULT_GEOMETRIC_SCALE,
        }
    }
    /// Returns the display name of this [`MinEdgeThicknessOperand`].
    #[must_use]
    pub fn name(&self) -> String {
        format!("MinEdgeThickness({})", self.node_id)
    }
    /// Returns the weight of this [`MinEdgeThicknessOperand`].
    #[must_use]
    pub const fn weight(&self) -> f64 {
        self.weight
    }
    /// Helper builder accepting a typed [`Length`] quantity for `min_thickness`.
    #[must_use]
    pub fn for_thickness(node_id: Uuid, min_thickness: Length, weight: f64) -> Self {
        Self::new(node_id, min_thickness.get::<meter>(), weight)
    }
    /// Returns the normalization scale of this [`MinEdgeThicknessOperand`].
    #[must_use]
    pub const fn scale(&self) -> f64 {
        self.scale
    }
}
/// Penalty operand discouraging ray clipping or total internal reflection losses at a detector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransmissionPenaltyOperand {
    /// Observable address of the ray-count or transmission metric on the target detector.
    pub address: super::address::ObservableAddress,
    /// Expected number of rays reaching the detector without clipping.
    pub expected_rays: f64,
    /// Relative weighting factor applied when fewer than `expected_rays` arrive.
    pub weight: f64,
}
impl TransmissionPenaltyOperand {
    /// Creates a new [`TransmissionPenaltyOperand`].
    #[must_use]
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
    /// Returns the display name of this [`TransmissionPenaltyOperand`].
    #[must_use]
    pub fn name(&self) -> String {
        format!("TransmissionPenalty({})", self.address)
    }
    /// Returns the weight of this [`TransmissionPenaltyOperand`].
    #[must_use]
    pub const fn weight(&self) -> f64 {
        self.weight
    }
}
/// Enumeration of all built-in merit operand types supported by the optimization engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StandardOperand {
    /// Target or inequality bound on a scalar detector observable or model parameter.
    Metric(MetricOperand),
    /// Upper bound on the cumulative length of multiple spacing/thickness parameters.
    MaxTotalLength(MaxTotalLengthOperand),
    /// Lower bound on the mechanical edge thickness of an optical lens.
    MinEdgeThickness(MinEdgeThicknessOperand),
    /// Penalty for lost or vignetted rays at a detector.
    TransmissionPenalty(TransmissionPenaltyOperand),
}
impl StandardOperand {
    /// Returns the display name of this [`StandardOperand`].
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
    pub const fn weight(&self) -> f64 {
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
