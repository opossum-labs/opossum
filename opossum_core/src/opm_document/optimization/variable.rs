use super::address::ParameterAddress;
use crate::error::{OpmResult, OpossumError};
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::meter};
/// Default relative step factor $\delta_{\text{rel}}$ used for finite-difference gradient estimation.
pub const DEFAULT_DELTA_REL: f64 = 1e-4;
/// Default minimum absolute step size $\delta_{\text{abs,min}}$ used when a parameter value is near zero.
pub const DEFAULT_DELTA_ABS_MIN: f64 = 1e-7;
/// Closed interval `[min, max]` defining box constraints on a parameter in SI base units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoxBounds {
    /// Lower bound (inclusive) in SI base units.
    pub min: f64,
    /// Upper bound (inclusive) in SI base units.
    pub max: f64,
}
impl BoxBounds {
    /// Creates a new [`BoxBounds`] in raw SI base units.
    ///
    /// # Errors
    ///
    /// Returns an error if either bound is `NaN` or if `min > max`.
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
    /// Creates a new [`BoxBounds`] from typed [`Length`] quantities, converting them to meters.
    ///
    /// # Errors
    ///
    /// Returns an error if either length is `NaN` or if `min > max`.
    pub fn from_lengths(min: Length, max: Length) -> OpmResult<Self> {
        Self::new(min.get::<meter>(), max.get::<meter>())
    }
    /// Returns `true` if `value` lies within `[self.min, self.max]`.
    #[must_use]
    pub fn contains(&self, value: f64) -> bool {
        value >= self.min && value <= self.max
    }
    /// Clamps `value` to the closed interval `[self.min, self.max]`.
    #[must_use]
    pub const fn clamp(&self, value: f64) -> f64 {
        value.clamp(self.min, self.max)
    }
    /// Computes a quadratic penalty for values outside `[self.min, self.max]`, or `0.0` if inside.
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
    /// Address of the optical property or connection distance being varied.
    pub address: ParameterAddress,
    /// Optional explicit start value in SI base units; if `None`, the current model value is read at solver startup.
    pub initial_value: Option<f64>,
    /// Optional lower and upper box constraints in SI base units.
    pub bounds: Option<BoxBounds>,
    /// Relative step size factor for finite-difference derivatives (defaults to [`DEFAULT_DELTA_REL`]).
    pub delta_rel: f64,
    /// Minimum absolute step size floor for finite-difference derivatives (defaults to [`DEFAULT_DELTA_ABS_MIN`]).
    pub delta_abs_min: f64,
}
impl FreeVariable {
    /// Creates a new [`FreeVariable`] targeting a specific [`ParameterAddress`] with default step settings.
    #[must_use]
    pub const fn new(address: ParameterAddress) -> Self {
        Self {
            address,
            initial_value: None,
            bounds: None,
            delta_rel: DEFAULT_DELTA_REL,
            delta_abs_min: DEFAULT_DELTA_ABS_MIN,
        }
    }
    /// Sets an explicit initial value in raw SI base units.
    #[must_use]
    pub const fn with_initial_value(mut self, value: f64) -> Self {
        self.initial_value = Some(value);
        self
    }
    /// Sets an explicit initial value from a typed [`Length`] quantity (stored in meters).
    #[must_use]
    pub fn with_initial_length(mut self, length: Length) -> Self {
        self.initial_value = Some(length.get::<meter>());
        self
    }
    /// Sets the [`BoxBounds`] constraining this variable.
    #[must_use]
    pub const fn with_bounds(mut self, bounds: BoxBounds) -> Self {
        self.bounds = Some(bounds);
        self
    }
    /// Sets lower and upper [`BoxBounds`] from typed [`Length`] quantities.
    ///
    /// # Errors
    ///
    /// Returns an error if either bound is `NaN` or if `min > max`.
    pub fn with_length_bounds(self, min: Length, max: Length) -> OpmResult<Self> {
        let bounds = BoxBounds::from_lengths(min, max)?;
        Ok(self.with_bounds(bounds))
    }
    /// Configures the relative and minimum absolute finite-difference step parameters.
    #[must_use]
    pub const fn with_step_config(mut self, delta_rel: f64, delta_abs_min: f64) -> Self {
        self.delta_rel = delta_rel;
        self.delta_abs_min = delta_abs_min;
        self
    }
    /// Computes the adaptive finite-difference step size $h = \max(\delta_{\text{abs,min}}, \delta_{\text{rel}} \cdot |x|)$.
    #[must_use]
    pub fn finite_difference_step(&self, current_val: f64) -> f64 {
        let rel_step = self.delta_rel * current_val.abs();
        self.delta_abs_min.max(rel_step)
    }
}
/// Method used to approximate numerical gradients via finite differences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FiniteDifferenceMethod {
    /// Central difference scheme: $\frac{f(x + h) - f(x - h)}{2h}$ ($O(h^2)$ accuracy, 2 evaluations per variable).
    #[default]
    Central,
    /// Forward difference scheme: $\frac{f(x + h) - f(x)}{h}$ ($O(h)$ accuracy, 1 additional evaluation per variable).
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
        let var = FreeVariable::new(addr);
        // Near zero: delta_abs_min dominates
        let step_at_zero = var.finite_difference_step(0.0);
        assert_eq!(step_at_zero, DEFAULT_DELTA_ABS_MIN);
        // Large values: delta_rel dominates
        let step_large = var.finite_difference_step(100.0);
        assert!((step_large - 100.0 * DEFAULT_DELTA_REL).abs() < 1e-12);
    }
}
