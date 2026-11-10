use crate::{address::MetricSource, reader::MetricReader};
use opossum_core::{
    core_optics::NodeAttrExt,
    error::{OpmResult, OpossumError},
    opm_document::OpmDocument,
};
use std::fmt::Debug;

pub use opossum_core::opm_document::optimization::operand::*;

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

impl Operand for TargetValueOperand {
    fn name(&self) -> &str {
        TargetValueOperand::name(self)
    }
    fn weight(&self) -> f64 {
        TargetValueOperand::weight(self)
    }
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        let value = ctx.read_source(&self.source)?;
        let normalized_diff = (value - self.target) / self.scale;
        Ok(self.weight * normalized_diff * normalized_diff)
    }
}

impl Operand for ThresholdOperand {
    fn name(&self) -> &str {
        ThresholdOperand::name(self)
    }
    fn weight(&self) -> f64 {
        ThresholdOperand::weight(self)
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

impl Operand for MaxTotalLengthOperand {
    fn name(&self) -> &str {
        MaxTotalLengthOperand::name(self)
    }
    fn weight(&self) -> f64 {
        MaxTotalLengthOperand::weight(self)
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

impl Operand for MinEdgeThicknessOperand {
    fn name(&self) -> &str {
        MinEdgeThicknessOperand::name(self)
    }
    fn weight(&self) -> f64 {
        MinEdgeThicknessOperand::weight(self)
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

impl Operand for TransmissionPenaltyOperand {
    fn name(&self) -> &str {
        TransmissionPenaltyOperand::name(self)
    }
    fn weight(&self) -> f64 {
        TransmissionPenaltyOperand::weight(self)
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

impl Operand for StandardOperand {
    fn name(&self) -> &str {
        StandardOperand::name(self)
    }
    fn weight(&self) -> f64 {
        StandardOperand::weight(self)
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
