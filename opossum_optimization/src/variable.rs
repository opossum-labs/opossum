use serde::{Deserialize, Serialize};

use crate::address::ParameterAddress;
use opossum_core::error::{OpmResult, OpossumError};

/// Default relative step size for finite differences (1e-4).
pub const DEFAULT_DELTA_REL: f64 = 1e-4;

/// Default minimum absolute step size for finite differences near zero (1e-7).
pub const DEFAULT_DELTA_ABS_MIN: f64 = 1e-7;

/// Box constraints defining valid parameter intervals [min, max].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoxBounds {
    /// Lower parameter bound (inclusive).
    pub min: f64,
    /// Upper parameter bound (inclusive).
    pub max: f64,
}

impl BoxBounds {
    /// Creates a new `BoxBounds` constraint.
    ///
    /// # Errors
    /// Returns an error if `min` is greater than `max` or either bound is NaN.
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

    /// Checks whether a given value lies within the bounds.
    #[must_use]
    pub fn contains(&self, value: f64) -> bool {
        value >= self.min && value <= self.max
    }

    /// Clamps a value into the valid interval [min, max].
    #[must_use]
    pub fn clamp(&self, value: f64) -> f64 {
        value.clamp(self.min, self.max)
    }

    /// Computes the quadratic penalty for bound violations[cite: 1].
    /// Returns 0.0 if the value is within bounds.
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

/// A free parameter directly manipulated by the numerical solver[cite: 1].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeVariable {
    /// Target address within the optical document[cite: 1].
    pub address: ParameterAddress,
    /// Human-readable label for debugging and logging.
    pub name: String,
    /// Initial value in SI base units.
    pub initial_value: f64,
    /// Optional box constraints[cite: 1].
    pub bounds: Option<BoxBounds>,
    /// Relative finite difference step size[cite: 1].
    pub delta_rel: f64,
    /// Minimum absolute finite difference step size[cite: 1].
    pub delta_abs_min: f64,
}

impl FreeVariable {
    /// Creates a new `FreeVariable` with default finite-difference step parameters[cite: 1].
    pub fn new(address: ParameterAddress, name: impl Into<String>, initial_value: f64) -> Self {
        Self {
            address,
            name: name.into(),
            initial_value,
            bounds: None,
            delta_rel: DEFAULT_DELTA_REL,
            delta_abs_min: DEFAULT_DELTA_ABS_MIN,
        }
    }

    /// Attaches box bounds to the variable.
    pub fn with_bounds(mut self, bounds: BoxBounds) -> Self {
        self.bounds = Some(bounds);
        self
    }

    /// Configures custom step sizes for finite differences.
    pub fn with_step_config(mut self, delta_rel: f64, delta_abs_min: f64) -> Self {
        self.delta_rel = delta_rel;
        self.delta_abs_min = delta_abs_min;
        self
    }

    /// Computes the adaptive finite difference step for gradient calculations:
    /// `Δx = max(δ_abs_min, δ_rel · |x|)`[cite: 1].
    #[must_use]
    pub fn finite_difference_step(&self, current_val: f64) -> f64 {
        let rel_step = self.delta_rel * current_val.abs();
        self.delta_abs_min.max(rel_step)
    }
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
        let var = FreeVariable::new(addr, "radius", 100.0);

        // Near zero: delta_abs_min dominates
        let step_at_zero = var.finite_difference_step(0.0);
        assert_eq!(step_at_zero, DEFAULT_DELTA_ABS_MIN);

        // Large values: delta_rel dominates
        let step_large = var.finite_difference_step(100.0);
        assert!((step_large - 100.0 * DEFAULT_DELTA_REL).abs() < 1e-12);
    }
}
