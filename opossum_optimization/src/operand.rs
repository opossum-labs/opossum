use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::{
    address::{MetricSource, ParameterAddress},
    reader::MetricReader,
};
use opossum_core::{error::OpmResult, opm_document::OpmDocument};

/// Context provided to operands during merit function evaluation.
pub struct EvaluationContext<'a> {
    /// Evaluated document containing post-analysis detector and model states.
    pub doc: &'a OpmDocument,
}

impl<'a> EvaluationContext<'a> {
    /// Creates a new evaluation context wrapping an optical document.
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

/// Extensible trait for individual optimization criteria (operands)[cite: 1].
pub trait Operand: Send + Sync + Debug {
    /// Human-readable name or description of the operand.
    fn name(&self) -> &str;

    /// Relative weight $w_i$ of this operand in the merit function[cite: 1].
    fn weight(&self) -> f64;

    /// Evaluates the operand contribution to the overall cost $\Phi(x)$[cite: 1].
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64>;
}

/// Standard operand targeting a specific value:
/// $\text{cost} = w \cdot \left( \frac{\text{value} - \text{target}}{\text{scale}} \right)^2$[cite: 1]
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

/// Geometric constraint operand penalizing if the sum of multiple parameter values exceeds an upper bound
///
/// $\text{cost} = w \cdot \left( \frac{\max(0, \sum p_i - L_{\max})}{\text{scale}} \right)^2$[cite: 1]
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

/// Standard serializable operand wrapper combining built-in operand variants[cite: 1].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StandardOperand {
    Target(TargetValueOperand),
    Threshold(ThresholdOperand),
    MaxTotalLength(MaxTotalLengthOperand),
}

impl Operand for StandardOperand {
    fn name(&self) -> &str {
        match self {
            Self::Target(op) => op.name(),
            Self::Threshold(op) => op.name(),
            Self::MaxTotalLength(op) => op.name(),
        }
    }

    fn weight(&self) -> f64 {
        match self {
            Self::Target(op) => op.weight(),
            Self::Threshold(op) => op.weight(),
            Self::MaxTotalLength(op) => op.weight(),
        }
    }

    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        match self {
            Self::Target(op) => op.evaluate(ctx),
            Self::Threshold(op) => op.evaluate(ctx),
            Self::MaxTotalLength(op) => op.evaluate(ctx),
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
