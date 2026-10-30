use crate::{
    address::{MetricSource, ObservableAddress, ParameterAddress},
    reader::MetricReader,
};
use opossum_core::{
    core_optics::NodeAttrExt,
    error::{OpmResult, OpossumError},
    opm_document::OpmDocument,
    utils::to_f64,
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use uuid::Uuid;

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

    /// Evaluates the operand contribution to the overall cost $\Phi(x)$.
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64>;

    /// Optional pre-flight validation against the pristine document model.
    ///
    /// Verifies that targeted nodes, analyzers, and required traits exist.
    fn validate(&self, _doc: &OpmDocument) -> OpmResult<()> {
        Ok(())
    }
}

/// Standard operand targeting a specific value:
/// $\text{cost} = w \cdot \left( \frac{\text{value} - \text{target}}{\text{scale}} \right)^2$
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetValueOperand {
    pub name: String,
    pub source: MetricSource,
    pub target: f64,
    pub weight: f64,
    pub scale: f64,
}

impl TargetValueOperand {
    /// Creates a new target value operand.
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
    /// Penalizes values exceeding limit: $\max(0, \text{val} - \text{limit})$.
    Upper,
    /// Penalizes values falling below limit: $\max(0, \text{limit} - \text{val})$.
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
    /// Creates a new threshold constraint operand.
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
///
/// $\text{cost} = w \cdot \left( \frac{\max(0, \sum `p_i` - L_{\max})}{\text{scale}} \right)^2$
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaxTotalLengthOperand {
    pub name: String,
    pub segments: Vec<ParameterAddress>,
    pub max_length: f64,
    pub weight: f64,
    pub scale: f64,
}

impl MaxTotalLengthOperand {
    /// Creates a new total length constraint operand.
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
            scale: 1.0e-3, // Standard scaling of 1 mm
        }
    }

    /// Overrides the normalization scale factor.
    #[must_use]
    pub fn with_scale(mut self, scale: f64) -> Self {
        if scale.abs() >= f64::EPSILON {
            self.scale = scale;
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
///
/// Uses the `EdgeThickness` trait of the target node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MinEdgeThicknessOperand {
    pub name: String,
    pub node_id: Uuid,
    pub min_thickness: f64,
    pub weight: f64,
    pub scale: f64,
}

impl MinEdgeThicknessOperand {
    /// Creates a new edge thickness constraint operand.
    #[must_use]
    pub fn new(name: impl Into<String>, node_id: Uuid, min_thickness: f64, weight: f64) -> Self {
        Self {
            name: name.into(),
            node_id,
            min_thickness,
            weight,
            scale: 1.0e-3, // Standard scaling of 1 mm
        }
    }

    /// Overrides the normalization scale factor.
    #[must_use]
    pub fn with_scale(mut self, scale: f64) -> Self {
        if scale.abs() >= f64::EPSILON {
            self.scale = scale;
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
///
/// $\text{cost} = w \cdot \left( \frac{\max(0, N_{\text{expected}} - N_{\text{valid}})}{N_{\text{expected}}} \right)^2$
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransmissionPenaltyOperand {
    pub name: String,
    pub address: ObservableAddress,
    pub expected_rays: f64,
    pub weight: f64,
}

impl TransmissionPenaltyOperand {
    /// Creates a new transmission penalty operand targeting a valid-ray detector metric.
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
        // Guard against division by zero if expected_rays is misconfigured
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
