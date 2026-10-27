use crate::address::ParameterAddress;
use opossum_core::error::{OpmResult, OpossumError};
use serde::{Deserialize, Serialize};

pub const DEFAULT_DELTA_REL: f64 = 1e-4;
pub const DEFAULT_DELTA_ABS_MIN: f64 = 1e-7;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoxBounds {
    pub min: f64,
    pub max: f64,
}

impl BoxBounds {
    pub fn new(min: f64, max: f64) -> OpmResult<Self> {
        if min.is_nan() || max.is_nan() {
            return Err(OpossumError::Other("Box bounds cannot be NaN".to_string()));
        }
        if min > max {
            return Err(OpossumError::Other(format!(
                "Lower bound ({min}) cannot exceed upper bound ({max})"
            )));
        }
        Ok(Self { min, max })
    }

    #[must_use]
    pub fn contains(&self, value: f64) -> bool {
        value >= self.min && value <= self.max
    }

    #[must_use]
    pub const fn clamp(&self, value: f64) -> f64 {
        value.clamp(self.min, self.max)
    }

    #[must_use]
    pub fn penalty(&self, value: f64) -> f64 {
        if value < self.min {
            let diff = self.min - value;
            diff * diff
        } else if value > self.max {
            let diff = value - self.max;
            diff * diff
        } else {
            0.0
        }
    }
}

/// A free parameter directly manipulated by the numerical solver.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeVariable {
    pub address: ParameterAddress,
    pub name: String,
    /// Initial value in SI base units. If None, derived automatically from the model.
    pub initial_value: Option<f64>,
    pub bounds: Option<BoxBounds>,
    pub delta_rel: f64,
    pub delta_abs_min: f64,
}

impl FreeVariable {
    /// Creates a new `FreeVariable` where the initial value is automatically read from the model[cite: 1].
    pub fn new(address: ParameterAddress, name: impl Into<String>) -> Self {
        Self {
            address,
            name: name.into(),
            initial_value: None,
            bounds: None,
            delta_rel: DEFAULT_DELTA_REL,
            delta_abs_min: DEFAULT_DELTA_ABS_MIN,
        }
    }

    /// Explicitly overrides the initial parameter value.
    pub const fn with_initial_value(mut self, value: f64) -> Self {
        self.initial_value = Some(value);
        self
    }

    pub const fn with_bounds(mut self, bounds: BoxBounds) -> Self {
        self.bounds = Some(bounds);
        self
    }

    pub const fn with_step_config(mut self, delta_rel: f64, delta_abs_min: f64) -> Self {
        self.delta_rel = delta_rel;
        self.delta_abs_min = delta_abs_min;
        self
    }

    #[must_use]
    pub fn finite_difference_step(&self, current_val: f64) -> f64 {
        let rel_step = self.delta_rel * current_val.abs();
        self.delta_abs_min.max(rel_step)
    }
}

/// Method used to approximate numerical gradients via finite differences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FiniteDifferenceMethod {
    /// Central difference scheme: (f(x + h) - f(x - h)) / (2h) (O(h^2) accuracy).
    #[default]
    Central,
    /// Forward difference scheme: (f(x + h) - f(x)) / h (O(h) accuracy, fewer evaluations).
    Forward,
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn box_bounds_validation_and_penalty() -> OpmResult<()> {
        assert!(BoxBounds::new(10.0, 5.0).is_err());
        assert!(BoxBounds::new(f64::NAN, 5.0).is_err());

        let bounds = BoxBounds::new(2.0, 8.0)?;
        assert!(bounds.contains(5.0));
        assert!(!bounds.contains(1.0));
        assert_eq!(bounds.clamp(0.0), 2.0);
        assert_eq!(bounds.clamp(10.0), 8.0);

        assert_eq!(bounds.penalty(5.0), 0.0);
        assert!((bounds.penalty(1.0) - 1.0).abs() < 1e-12);
        assert!((bounds.penalty(10.0) - 4.0).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn adaptive_step_calculation() {
        let addr = ParameterAddress::NodeProperty {
            node_id: Uuid::new_v4(),
            property_name: "radius".into(),
        };
        let var = FreeVariable::new(addr, "radius");

        // Near zero: delta_abs_min dominates
        let step_at_zero = var.finite_difference_step(0.0);
        assert_eq!(step_at_zero, DEFAULT_DELTA_ABS_MIN);

        // Large values: delta_rel dominates
        let step_large = var.finite_difference_step(100.0);
        assert!((step_large - 100.0 * DEFAULT_DELTA_REL).abs() < 1e-12);
    }
}
