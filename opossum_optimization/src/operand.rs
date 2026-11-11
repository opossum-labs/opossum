use crate::{address::MetricSource, reader::MetricReader};
pub use opossum_core::opm_document::optimization::operand::*;
use opossum_core::{
    core_optics::NodeAttrExt,
    error::{OpmResult, OpossumError},
    opm_document::OpmDocument,
};
use std::fmt::Debug;

/// Context provided to operands during merit function evaluation.
pub struct EvaluationContext<'a> {
    pub doc: &'a OpmDocument,
}

impl<'a> EvaluationContext<'a> {
    #[must_use]
    pub const fn new(doc: &'a OpmDocument) -> Self {
        Self { doc }
    }

    pub fn read_source(&self, source: &MetricSource) -> OpmResult<f64> {
        match source {
            MetricSource::Observable(addr) => MetricReader::read_metric(self.doc, addr),
            MetricSource::Parameter(addr) => MetricReader::read_parameter_value(self.doc, addr),
        }
    }
}

pub trait Operand: Send + Sync + Debug {
    /// Human-readable descriptor of the operand for logging and diagnostics.
    fn name(&self) -> String;
    fn weight(&self) -> f64;
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64>;
    fn residual(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        self.evaluate(ctx).map(|c| c.max(0.0).sqrt())
    }
    fn validate(&self, _doc: &OpmDocument) -> OpmResult<()> {
        Ok(())
    }
}

impl Operand for MetricOperand {
    fn name(&self) -> String {
        MetricOperand::name(self)
    }
    fn weight(&self) -> f64 {
        MetricOperand::weight(self)
    }
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        let value = ctx.read_source(&self.source)?;
        let violation = match self.mode {
            OperandMode::Equal => value - self.target,
            OperandMode::UpperLimit => (value - self.target).max(0.0),
            OperandMode::LowerLimit => (self.target - value).max(0.0),
        };
        let normalized = violation / self.scale;
        Ok(self.weight * normalized * normalized)
    }
    fn residual(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        let value = ctx.read_source(&self.source)?;
        let diff = match self.mode {
            OperandMode::Equal => value - self.target,
            OperandMode::UpperLimit => (value - self.target).max(0.0),
            OperandMode::LowerLimit => (self.target - value).max(0.0),
        };
        Ok(self.weight.sqrt() * (diff / self.scale))
    }
}

impl Operand for MaxTotalLengthOperand {
    fn name(&self) -> String {
        MaxTotalLengthOperand::name(self).into_owned()
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
    fn name(&self) -> String {
        MinEdgeThicknessOperand::name(self)
    }
    fn weight(&self) -> f64 {
        MinEdgeThicknessOperand::weight(self)
    }
    fn validate(&self, doc: &OpmDocument) -> OpmResult<()> {
        let (node_ref, _) = doc.scenery().node_recursive(self.node_id)?;
        if node_ref.as_edge_thickness().is_none() {
            return Err(OpossumError::Other(format!(
                "Node '{}' ({}) does not support edge thickness evaluation",
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
    fn name(&self) -> String {
        TransmissionPenaltyOperand::name(self)
    }
    fn weight(&self) -> f64 {
        TransmissionPenaltyOperand::weight(self)
    }
    fn validate(&self, doc: &OpmDocument) -> OpmResult<()> {
        let _ = doc.scenery().node_recursive(self.address.node_id)?;
        if !doc.analyzers().contains_key(&self.address.analyzer_id) {
            return Err(OpossumError::Other(format!(
                "Analyzer '{}' not found in document",
                self.address.analyzer_id
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
    fn name(&self) -> String {
        StandardOperand::name(self)
    }
    fn weight(&self) -> f64 {
        StandardOperand::weight(self)
    }
    fn validate(&self, doc: &OpmDocument) -> OpmResult<()> {
        match self {
            Self::Metric(op) => op.validate(doc),
            Self::MaxTotalLength(op) => op.validate(doc),
            Self::MinEdgeThickness(op) => op.validate(doc),
            Self::TransmissionPenalty(op) => op.validate(doc),
        }
    }
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        match self {
            Self::Metric(op) => op.evaluate(ctx),
            Self::MaxTotalLength(op) => op.evaluate(ctx),
            Self::MinEdgeThickness(op) => op.evaluate(ctx),
            Self::TransmissionPenalty(op) => op.evaluate(ctx),
        }
    }
    fn residual(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        match self {
            Self::Metric(op) => op.residual(ctx),
            Self::MaxTotalLength(op) => op.residual(ctx),
            Self::MinEdgeThickness(op) => op.residual(ctx),
            Self::TransmissionPenalty(op) => op.residual(ctx),
        }
    }
}
