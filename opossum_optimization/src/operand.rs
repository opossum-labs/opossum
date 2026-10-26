use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::{address::ObservableAddress, reader::MetricReader};
use opossum_core::{error::OpmResult, opm_document::OpmDocument};

/// Context provided to operands during merit function evaluation.
pub struct EvaluationContext<'a> {
    /// Evaluated document containing post-analysis detector states.
    pub doc: &'a OpmDocument,
}

impl<'a> EvaluationContext<'a> {
    /// Creates a new evaluation context wrapping an optical document.
    pub fn new(doc: &'a OpmDocument) -> Self {
        Self { doc }
    }

    /// Reads a scalar metric from a detector node inside the document.
    pub fn read_metric(&self, address: &ObservableAddress) -> OpmResult<f64> {
        MetricReader::read_metric(self.doc, address)
    }
}

/// Extensible trait for individual optimization criteria (operands).
pub trait Operand: Send + Sync + Debug {
    /// Human-readable name or description of the operand.
    fn name(&self) -> &str;

    /// Relative weight $w_i$ of this operand in the merit function.
    fn weight(&self) -> f64;

    /// Evaluates the operand contribution to the overall cost $\Phi(x)$.
    fn evaluate(&self, ctx: &EvaluationContext) -> OpmResult<f64>;
}

/// Standard operand targeting a specific scalar value:
/// $\text{cost} = w \cdot \left( \frac{\text{value} - \text{target}}{\text{scale}} \right)^2$
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetValueOperand {
    pub name: String,
    pub address: ObservableAddress,
    pub target: f64,
    pub weight: f64,
    pub scale: f64,
}

impl TargetValueOperand {
    /// Creates a new target value operand.
    pub fn new(
        name: impl Into<String>,
        address: ObservableAddress,
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
            address,
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

    fn evaluate(&self, ctx: &EvaluationContext) -> OpmResult<f64> {
        let value = ctx.read_metric(&self.address)?;
        let normalized_diff = (value - self.target) / self.scale;
        Ok(self.weight * normalized_diff * normalized_diff)
    }
}

/// Threshold operand penalizing values exceeding an upper or falling below a lower limit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ThresholdDirection {
    /// Penalizes values greater than limit: $\max(0, \text{val} - \text{limit})$.
    Upper,
    /// Penalizes values smaller than limit: $\max(0, \text{limit} - \text{val})$.
    Lower,
}

/// One-sided inequality constraint operand for optical metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThresholdOperand {
    pub name: String,
    pub address: ObservableAddress,
    pub limit: f64,
    pub direction: ThresholdDirection,
    pub weight: f64,
    pub scale: f64,
}

impl ThresholdOperand {
    /// Creates a new threshold constraint operand.
    pub fn new(
        name: impl Into<String>,
        address: ObservableAddress,
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
            address,
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

    fn evaluate(&self, ctx: &EvaluationContext) -> OpmResult<f64> {
        let value = ctx.read_metric(&self.address)?;
        let violation = match self.direction {
            ThresholdDirection::Upper => (value - self.limit).max(0.0),
            ThresholdDirection::Lower => (self.limit - value).max(0.0),
        };
        let normalized = violation / self.scale;
        Ok(self.weight * normalized * normalized)
    }
}

/// Standard serializable operand wrapper combining built-in operand variants[cite: 1].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StandardOperand {
    Target(TargetValueOperand),
    Threshold(ThresholdOperand),
}

impl Operand for StandardOperand {
    fn name(&self) -> &str {
        match self {
            Self::Target(op) => op.name(),
            Self::Threshold(op) => op.name(),
        }
    }

    fn weight(&self) -> f64 {
        match self {
            Self::Target(op) => op.weight(),
            Self::Threshold(op) => op.weight(),
        }
    }

    fn evaluate(&self, ctx: &EvaluationContext) -> OpmResult<f64> {
        match self {
            Self::Target(op) => op.evaluate(ctx),
            Self::Threshold(op) => op.evaluate(ctx),
        }
    }
}
