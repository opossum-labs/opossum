use super::address::ParameterAddress;
use crate::error::{OpmResult, OpossumError};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de::Visitor};
use std::fmt;
use uom::si::{f64::Length, length::meter};

/// Default relative step factor $\delta_{\text{rel}}$ used for finite-difference gradient estimation.
pub const DEFAULT_DELTA_REL: f64 = 1e-4;
/// Default minimum absolute step size $\delta_{\text{abs,min}}$ used when a parameter value is near zero.
pub const DEFAULT_DELTA_ABS_MIN: f64 = 1e-7;

struct BoundVisitor {
    default_infinite: f64,
}

impl<'de> Visitor<'de> for BoundVisitor {
    type Value = f64;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a floating-point bound or null for infinity")
    }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E> {
        Ok(v)
    }

    #[allow(clippy::cast_precision_loss)]
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
        Ok(v as f64)
    }

    #[allow(clippy::cast_precision_loss)]
    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
        Ok(v as f64)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(self.default_infinite)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(self.default_infinite)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

fn deserialize_lower_bound<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(BoundVisitor {
        default_infinite: f64::NEG_INFINITY,
    })
}

fn deserialize_upper_bound<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(BoundVisitor {
        default_infinite: f64::INFINITY,
    })
}

/// Closed interval `[min, max]` defining box constraints on a parameter in SI base units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BoxBounds {
    /// Lower bound (inclusive) in SI base units (may be `f64::NEG_INFINITY`).
    #[serde(deserialize_with = "deserialize_lower_bound")]
    pub min: f64,
    /// Upper bound (inclusive) in SI base units (may be `f64::INFINITY`).
    #[serde(deserialize_with = "deserialize_upper_bound")]
    pub max: f64,
}

impl Default for BoxBounds {
    fn default() -> Self {
        Self::unbounded()
    }
}

impl BoxBounds {
    /// Creates unbounded constraints `[-inf, +inf]`.
    #[must_use]
    pub const fn unbounded() -> Self {
        Self {
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
        }
    }

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

    /// Returns `true` if both `min` and `max` are finite numbers.
    #[must_use]
    pub const fn is_finite(&self) -> bool {
        self.min.is_finite() && self.max.is_finite()
    }

    /// Returns the interval width `max - min` if both bounds are finite, or `None` otherwise.
    #[must_use]
    pub const fn finite_span(&self) -> Option<f64> {
        if self.is_finite() {
            Some(self.max - self.min)
        } else {
            None
        }
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FreeVariable {
    /// Address of the optical property or connection distance being varied.
    pub address: ParameterAddress,
    /// Lower and upper box constraints in SI base units (defaults to `[-inf, +inf]`).
    #[serde(default)]
    pub bounds: BoxBounds,
    /// Relative step size factor for finite-difference derivatives (defaults to [`DEFAULT_DELTA_REL`]).
    pub delta_rel: f64,
    /// Minimum absolute step size floor for finite-difference derivatives (defaults to [`DEFAULT_DELTA_ABS_MIN`]).
    pub delta_abs_min: f64,
}

impl FreeVariable {
    /// Creates a new unbounded [`FreeVariable`] targeting a specific [`ParameterAddress`] with default step settings.
    #[must_use]
    pub const fn new(address: ParameterAddress) -> Self {
        Self {
            address,
            bounds: BoxBounds::unbounded(),
            delta_rel: DEFAULT_DELTA_REL,
            delta_abs_min: DEFAULT_DELTA_ABS_MIN,
        }
    }

    /// Sets the [`BoxBounds`] constraining this variable.
    #[must_use]
    pub const fn with_bounds(mut self, bounds: BoxBounds) -> Self {
        self.bounds = bounds;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
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
