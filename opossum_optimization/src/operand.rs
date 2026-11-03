use crate::{
    address::{MetricSource, ObservableAddress, ParameterAddress},
    reader::MetricReader,
};
use opossum_core::{
    core_optics::{NodeAttrExt, observable::MetricDimension},
    error::{OpmResult, OpossumError},
    opm_document::OpmDocument,
    utils::to_f64,
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use uom::si::{
    energy::joule,
    f64::{Energy, Length, RadiantExposure},
    length::meter,
    radiant_exposure::joule_per_square_meter,
};
use uuid::Uuid;

/// Default scale constants for optical observables in SI base units (meters).
pub const DEFAULT_SPOT_SCALE: f64 = 1.0e-6; // 1.0 micrometer
pub const DEFAULT_WAVEFRONT_SCALE: f64 = 1.0e-7; // 100.0 nanometers (~ lambda / 10)
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

/// Context provided to operands during merit function evaluation.
pub struct EvaluationContext<'a> {
    /// Evaluated document containing post-analysis detector and model states.
    pub doc: &'a OpmDocument,
}

impl<'a> EvaluationContext<'a> {
    /// Creates a new evaluation context wrapping an optical document.
    #[must_use]
    pub const fn new(doc: &'a OpmDocument) -> Self {
        Self { doc }
    }

    /// Evaluates any quantity source (either detector observable or model parameter).
    ///
    /// # Errors
    ///
    /// This function returns an error if the given [`MetricSource`] contains invalid data
    pub fn read_source(&self, source: &MetricSource) -> OpmResult<f64> {
        match source {
            MetricSource::Observable(addr) => MetricReader::read_metric(self.doc, addr),
            MetricSource::Parameter(addr) => MetricReader::read_parameter_value(self.doc, addr),
        }
    }
}

/// Extensible trait for individual optimization criteria (operands).
pub trait Operand: Send + Sync + Debug {
    /// Human-readable name or description of the operand.
    fn name(&self) -> &str;
    /// Relative weight `w_i` of this operand in the merit function.
    fn weight(&self) -> f64;
    /// Evaluates the operand contribution to the overall cost Phi(x).
    ///
    /// # Errors
    ///
    /// This function returns an error if the underlying implementation fails.
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64>;
    /// Optional pre-flight validation against the pristine document model.
    ///
    /// # Errors
    ///
    /// This function returns an error if the validation fails.
    fn validate(&self, _doc: &OpmDocument) -> OpmResult<()> {
        Ok(())
    }
}

/// Standard operand targeting a specific value:
/// cost = weight * ((value - target) / scale)^2
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetValueOperand {
    pub name: String,
    pub source: MetricSource,
    pub target: f64,
    pub weight: f64,
    pub scale: f64,
}

impl TargetValueOperand {
    /// Creates a raw target value operand using SI base units.
    pub fn new(
        name: impl Into<String>,
        source: impl Into<MetricSource>,
        target: f64,
        weight: f64,
        scale: f64,
    ) -> Self {
        let scale = if scale.abs() < f64::EPSILON {
            1.0
        } else {
            scale
        };
        Self {
            name: name.into(),
            source: source.into(),
            target,
            weight,
            scale,
        }
    }

    /// Creates an operand targeting a spot size metric with automatic micrometer default scaling.
    pub fn for_spot_radius(
        name: impl Into<String>,
        source: impl Into<MetricSource>,
        target: Length,
        weight: f64,
    ) -> Self {
        Self::new(
            name,
            source,
            target.get::<meter>(),
            weight,
            DEFAULT_SPOT_SCALE,
        )
    }

    /// Creates an operand targeting wavefront aberrations with automatic nanometer default scaling.
    pub fn for_wavefront(
        name: impl Into<String>,
        source: impl Into<MetricSource>,
        target: Length,
        weight: f64,
    ) -> Self {
        Self::new(
            name,
            source,
            target.get::<meter>(),
            weight,
            DEFAULT_WAVEFRONT_SCALE,
        )
    }
    /// Creates a target operand for wavefront error specified in waves (lambda).
    /// Default scale is 0.02 waves (lambda / 50).
    pub fn for_wavefront_waves(
        name: impl Into<String>,
        source: impl Into<MetricSource>,
        target_waves: f64,
        weight: f64,
    ) -> Self {
        Self::new(name, source, target_waves, weight, 0.02)
    }

    /// Creates a target operand for wavefront error in waves with a custom scale tolerance.
    pub fn for_wavefront_waves_with_scale(
        name: impl Into<String>,
        source: impl Into<MetricSource>,
        target_waves: f64,
        scale_waves: f64,
        weight: f64,
    ) -> Self {
        Self::new(name, source, target_waves, weight, scale_waves)
    }

    /// Creates a general length operand with automatic millimeter default scaling (e.g. beam spacing, lengths).
    pub fn for_length(
        name: impl Into<String>,
        source: impl Into<MetricSource>,
        target: Length,
        weight: f64,
    ) -> Self {
        Self::new(
            name,
            source,
            target.get::<meter>(),
            weight,
            DEFAULT_GEOMETRIC_SCALE,
        )
    }

    /// Creates a typed length operand with an explicit tolerance scale factor.
    pub fn for_length_with_scale(
        name: impl Into<String>,
        source: impl Into<MetricSource>,
        target: Length,
        scale: Length,
        weight: f64,
    ) -> Self {
        Self::new(
            name,
            source,
            target.get::<meter>(),
            weight,
            scale.get::<meter>(),
        )
    }
    /// Creates a target operand for optical energy observables with automatic 1.0 mJ default scaling.
    pub fn for_energy(
        name: impl Into<String>,
        source: impl Into<MetricSource>,
        target: Energy,
        weight: f64,
    ) -> Self {
        Self::new(
            name,
            source,
            target.get::<joule>(),
            weight,
            1.0e-3, // 1.0 mJ default normalization scale
        )
    }

    /// Creates a target operand for optical energy with explicit tolerance scaling.
    pub fn for_energy_with_scale(
        name: impl Into<String>,
        source: impl Into<MetricSource>,
        target: Energy,
        scale: Energy,
        weight: f64,
    ) -> Self {
        Self::new(
            name,
            source,
            target.get::<joule>(),
            weight,
            scale.get::<joule>(),
        )
    }

    /// Creates a target operand for physical optical path difference (OPD).
    /// Default scale is 100 nm (0.1 um).
    pub fn for_wavefront_opd(
        name: impl Into<String>,
        source: impl Into<MetricSource>,
        target_opd: Length,
        weight: f64,
    ) -> Self {
        Self::new(
            name,
            source,
            target_opd.get::<uom::si::length::meter>(),
            weight,
            DEFAULT_WAVEFRONT_SCALE, // 1.0e-7 m
        )
    }
    /// Creates a target operand for fluence metrics with automatic default scaling (1.0 J/cm² = 10,000 J/m²).
    pub fn for_fluence(
        name: impl Into<String>,
        source: impl Into<MetricSource>,
        target: RadiantExposure,
        weight: f64,
    ) -> Self {
        Self::new(
            name,
            source,
            target.get::<joule_per_square_meter>(),
            weight,
            1.0e4, // 1.0 J/cm^2 default normalization scale
        )
    }
}

impl Operand for TargetValueOperand {
    fn name(&self) -> &str {
        &self.name
    }
    fn weight(&self) -> f64 {
        self.weight
    }
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        let value = ctx.read_source(&self.source)?;
        let normalized_diff = (value - self.target) / self.scale;
        Ok(self.weight * normalized_diff * normalized_diff)
    }
}

/// Direction for threshold inequality bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThresholdDirection {
    /// Penalizes values exceeding limit: max(0, val - limit).
    Upper,
    /// Penalizes values falling below limit: max(0, limit - val).
    Lower,
}

/// One-sided inequality constraint operand for optical metrics or parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThresholdOperand {
    pub name: String,
    pub source: MetricSource,
    pub limit: f64,
    pub direction: ThresholdDirection,
    pub weight: f64,
    pub scale: f64,
}

impl ThresholdOperand {
    /// Creates a raw threshold constraint operand.
    pub fn new(
        name: impl Into<String>,
        source: impl Into<MetricSource>,
        limit: f64,
        direction: ThresholdDirection,
        weight: f64,
        scale: f64,
    ) -> Self {
        let scale = if scale.abs() < f64::EPSILON {
            1.0
        } else {
            scale
        };
        Self {
            name: name.into(),
            source: source.into(),
            limit,
            direction,
            weight,
            scale,
        }
    }

    /// Creates a threshold constraint on a spot size metric with automatic micrometer scaling.
    pub fn for_spot_radius(
        name: impl Into<String>,
        source: impl Into<MetricSource>,
        limit: Length,
        direction: ThresholdDirection,
        weight: f64,
    ) -> Self {
        Self::new(
            name,
            source,
            limit.get::<meter>(),
            direction,
            weight,
            DEFAULT_SPOT_SCALE,
        )
    }

    /// Creates a threshold constraint on a geometric length with automatic millimeter scaling.
    pub fn for_length(
        name: impl Into<String>,
        source: impl Into<MetricSource>,
        limit: Length,
        direction: ThresholdDirection,
        weight: f64,
    ) -> Self {
        Self::new(
            name,
            source,
            limit.get::<meter>(),
            direction,
            weight,
            DEFAULT_GEOMETRIC_SCALE,
        )
    }
    /// Creates an upper threshold constraint for fluence (e.g. enforcing LIDT laser damage thresholds).
    pub fn for_max_fluence(
        name: impl Into<String>,
        source: impl Into<MetricSource>,
        limit: RadiantExposure,
        weight: f64,
    ) -> Self {
        Self::new(
            name,
            source,
            limit.get::<joule_per_square_meter>(),
            ThresholdDirection::Upper,
            weight,
            1.0e4, // 1.0 J/cm^2 scale
        )
    }
}

impl Operand for ThresholdOperand {
    fn name(&self) -> &str {
        &self.name
    }
    fn weight(&self) -> f64 {
        self.weight
    }
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        let value = ctx.read_source(&self.source)?;
        let violation = match self.direction {
            ThresholdDirection::Upper => (value - self.limit).max(0.0),
            ThresholdDirection::Lower => (self.limit - value).max(0.0),
        };
        let normalized = violation / self.scale;
        Ok(self.weight * normalized * normalized)
    }
}

/// Geometric constraint operand penalizing if the sum of multiple parameter values exceeds an upper bound.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaxTotalLengthOperand {
    pub name: String,
    pub segments: Vec<ParameterAddress>,
    pub max_length: f64,
    pub weight: f64,
    pub scale: f64,
}

impl MaxTotalLengthOperand {
    /// Creates a new total length constraint operand with raw floats.
    pub fn new(
        name: impl Into<String>,
        segments: Vec<ParameterAddress>,
        max_length: f64,
        weight: f64,
    ) -> Self {
        Self {
            name: name.into(),
            segments,
            max_length,
            weight,
            scale: DEFAULT_GEOMETRIC_SCALE,
        }
    }

    /// Creates a new total length constraint operand using typed Length quantities.
    pub fn for_length(
        name: impl Into<String>,
        segments: Vec<ParameterAddress>,
        max_length: Length,
        weight: f64,
    ) -> Self {
        Self::new(name, segments, max_length.get::<meter>(), weight)
    }

    /// Overrides the normalization scale factor with a typed Length.
    #[must_use]
    pub fn with_length_scale(mut self, scale: Length) -> Self {
        let scale_m = scale.get::<meter>();
        if scale_m.abs() >= f64::EPSILON {
            self.scale = scale_m;
        }
        self
    }
}

impl Operand for MaxTotalLengthOperand {
    fn name(&self) -> &str {
        &self.name
    }
    fn weight(&self) -> f64 {
        self.weight
    }
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        let mut total = 0.0;
        for seg in &self.segments {
            total += MetricReader::read_parameter_value(ctx.doc, seg)?;
        }
        let violation = (total - self.max_length).max(0.0);
        let normalized = violation / self.scale;
        Ok(self.weight * normalized * normalized)
    }
}

/// Geometric constraint penalizing optical components whose edge thickness drops below a lower bound.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MinEdgeThicknessOperand {
    pub name: String,
    pub node_id: Uuid,
    pub min_thickness: f64,
    pub weight: f64,
    pub scale: f64,
}

impl MinEdgeThicknessOperand {
    /// Creates a new edge thickness constraint operand with raw floats.
    #[must_use]
    pub fn new(name: impl Into<String>, node_id: Uuid, min_thickness: f64, weight: f64) -> Self {
        Self {
            name: name.into(),
            node_id,
            min_thickness,
            weight,
            scale: DEFAULT_GEOMETRIC_SCALE,
        }
    }

    /// Creates a new edge thickness constraint operand using typed Length.
    #[must_use]
    pub fn for_thickness(
        name: impl Into<String>,
        node_id: Uuid,
        min_thickness: Length,
        weight: f64,
    ) -> Self {
        Self::new(name, node_id, min_thickness.get::<meter>(), weight)
    }

    /// Overrides the normalization scale factor with a typed Length.
    #[must_use]
    pub fn with_length_scale(mut self, scale: Length) -> Self {
        let scale_m = scale.get::<meter>();
        if scale_m.abs() >= f64::EPSILON {
            self.scale = scale_m;
        }
        self
    }
}

impl Operand for MinEdgeThicknessOperand {
    fn name(&self) -> &str {
        &self.name
    }
    fn weight(&self) -> f64 {
        self.weight
    }
    fn validate(&self, doc: &OpmDocument) -> OpmResult<()> {
        let (node_ref, _) = doc.scenery().node_recursive(self.node_id)?;
        if node_ref.as_edge_thickness().is_none() {
            return Err(OpossumError::Other(format!(
                "MinEdgeThicknessOperand '{}': Node '{}' ({}) does not support edge thickness evaluation",
                self.name,
                node_ref.name(),
                node_ref.node_type()
            )));
        }
        Ok(())
    }
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        let (node_ref, _) = ctx.doc.scenery().node_recursive(self.node_id)?;
        let edge_thickness = node_ref
            .as_edge_thickness()
            .ok_or_else(|| {
                OpossumError::Other(format!(
                    "Node '{}' does not implement EdgeThickness",
                    node_ref.name()
                ))
            })?
            .min_edge_thickness()?
            .value;
        let violation = (self.min_thickness - edge_thickness).max(0.0);
        let normalized = violation / self.scale;
        Ok(self.weight * normalized * normalized)
    }
}

/// Penalizes ray loss at a detector when the number of valid rays drops below the expected count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransmissionPenaltyOperand {
    pub name: String,
    pub address: ObservableAddress,
    pub expected_rays: f64,
    pub weight: f64,
}

impl TransmissionPenaltyOperand {
    pub fn new(
        name: impl Into<String>,
        address: ObservableAddress,
        expected_rays: usize,
        weight: f64,
    ) -> Self {
        Self {
            name: name.into(),
            address,
            expected_rays: to_f64(expected_rays),
            weight,
        }
    }
}

impl Operand for TransmissionPenaltyOperand {
    fn name(&self) -> &str {
        &self.name
    }
    fn weight(&self) -> f64 {
        self.weight
    }
    fn validate(&self, doc: &OpmDocument) -> OpmResult<()> {
        let _ = doc.scenery().node_recursive(self.address.node_id)?;
        if !doc.analyzers().contains_key(&self.address.analyzer_id) {
            return Err(OpossumError::Other(format!(
                "TransmissionPenaltyOperand '{}': Analyzer '{}' not found in document",
                self.name, self.address.analyzer_id
            )));
        }
        Ok(())
    }
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        if self.expected_rays <= 0.0 {
            return Ok(0.0);
        }
        let valid_rays = MetricReader::read_metric(ctx.doc, &self.address)?;
        if valid_rays >= self.expected_rays {
            return Ok(0.0);
        }
        let loss_ratio = (self.expected_rays - valid_rays) / self.expected_rays;
        Ok(self.weight * loss_ratio * loss_ratio)
    }
}

/// Standard serializable operand wrapper combining built-in operand variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StandardOperand {
    Target(TargetValueOperand),
    Threshold(ThresholdOperand),
    MaxTotalLength(MaxTotalLengthOperand),
    MinEdgeThickness(MinEdgeThicknessOperand),
    TransmissionPenalty(TransmissionPenaltyOperand),
}

impl Operand for StandardOperand {
    fn name(&self) -> &str {
        match self {
            Self::Target(op) => op.name(),
            Self::Threshold(op) => op.name(),
            Self::MaxTotalLength(op) => op.name(),
            Self::MinEdgeThickness(op) => op.name(),
            Self::TransmissionPenalty(op) => op.name(),
        }
    }
    fn weight(&self) -> f64 {
        match self {
            Self::Target(op) => op.weight(),
            Self::Threshold(op) => op.weight(),
            Self::MaxTotalLength(op) => op.weight(),
            Self::MinEdgeThickness(op) => op.weight(),
            Self::TransmissionPenalty(op) => op.weight(),
        }
    }
    fn validate(&self, doc: &OpmDocument) -> OpmResult<()> {
        match self {
            Self::Target(op) => op.validate(doc),
            Self::Threshold(op) => op.validate(doc),
            Self::MaxTotalLength(op) => op.validate(doc),
            Self::MinEdgeThickness(op) => op.validate(doc),
            Self::TransmissionPenalty(op) => op.validate(doc),
        }
    }
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        match self {
            Self::Target(op) => op.evaluate(ctx),
            Self::Threshold(op) => op.evaluate(ctx),
            Self::MaxTotalLength(op) => op.evaluate(ctx),
            Self::MinEdgeThickness(op) => op.evaluate(ctx),
            Self::TransmissionPenalty(op) => op.evaluate(ctx),
        }
    }
}

impl From<TargetValueOperand> for StandardOperand {
    fn from(op: TargetValueOperand) -> Self {
        Self::Target(op)
    }
}
impl From<ThresholdOperand> for StandardOperand {
    fn from(op: ThresholdOperand) -> Self {
        Self::Threshold(op)
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
