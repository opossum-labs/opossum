//! Circular, hexapolar distribution
use std::f64::consts::PI;

use crate::{
    error::OpmResult,
    generic_validators::{AllFinite, AllPositive},
    meter, millimeter, validated, validated_type,
};

use super::PositionDistribution;
use nalgebra::{Point3, Vector3};
use num::{ToPrimitive, Zero};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

/// Circular, hexapolar distribution
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Copy, EnsureValidated)]
pub struct HexagonalTiling {
    #[validate(skip)]
    nr_of_hex_along_radius: u8,
    radius: validated_type!(Length, AllPositive && AllFinite),
}
impl HexagonalTiling {
    /// Create a new [`HexagonalTiling`] distribution generator.
    ///
    /// If the given radius is zero and / or `nr_of_rings` is zero only the central point at (0,0) is generated.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///  - the given `radius` is negative or not finite.
    pub fn new(radius: Length, nr_of_hex_along_radius: u8) -> OpmResult<Self> {
        let mut hexagonal = Self::default();
        hexagonal.set_radius(radius)?;
        hexagonal.set_nr_of_hex_along_radius(nr_of_hex_along_radius);
        Ok(hexagonal)
    }
    /// Returns the radius of the hexagonal tiling distribution.
    ///
    /// # Returns
    ///
    /// The radius as a `Length`.
    #[must_use]
    pub fn radius(&self) -> Length {
        *self.radius.get()
    }

    /// Returns the number of hexagons along the radius.
    ///
    /// # Returns
    ///
    /// The number of hexagons as a `u8`.
    #[must_use]
    pub const fn nr_of_hex_along_radius(&self) -> u8 {
        self.nr_of_hex_along_radius
    }
    /// Sets the radius of the hexagonal tiling distribution.
    ///
    /// # Parameters
    ///
    /// * `radius` - The new radius as a `Length`.
    ///
    /// # Side Effects
    ///
    /// Updates the current radius.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_radius(&mut self, radius: Length) -> OpmResult<()> {
        self.radius.set(radius)?;
        Ok(())
    }

    /// Sets the number of hexagons along the radius.
    ///
    /// # Parameters
    ///
    /// * `nr_of_hex_along_radius` - The new number of hexagons as a `u8`.
    ///
    /// # Side Effects
    ///
    /// Updates the current number of hexagons.
    pub const fn set_nr_of_hex_along_radius(&mut self, nr_of_hex_along_radius: u8) {
        self.nr_of_hex_along_radius = nr_of_hex_along_radius;
    }
}

impl Default for HexagonalTiling {
    fn default() -> Self {
        Self {
            nr_of_hex_along_radius: 7,
            radius: validated!(millimeter!(5.), AllPositive && AllFinite).unwrap(),
        }
    }
}

impl PositionDistribution for HexagonalTiling {
    fn generate(&self) -> Vec<Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::new();
        // Add center point
        points.push(meter!(0.0, 0.0, 0.0));

        let radius_step = *self.radius.get() / self.nr_of_hex_along_radius.to_f64().unwrap();
        let mut i = 1;
        let border_radius = *self.radius.get() * 5.0f64.mul_add(f64::EPSILON, 1.);
        loop {
            let mut all_outside_radius = true;
            let mut hex = meter!(0.0, 0.0, 0.0);
            hex.x = radius_step * i.to_f64().unwrap();
            for j in 0_u8..6 {
                let angle = PI / 3. * (2. + j.to_f64().unwrap());
                let shift_vec = Vector3::new(
                    f64::cos(angle) * radius_step,
                    f64::sin(angle) * radius_step,
                    Length::zero(),
                );
                for _k in 0_u8..i {
                    if ((hex.x) * (hex.x) + (hex.y) * (hex.y)).sqrt() <= border_radius {
                        points.push(hex);
                        all_outside_radius = false;
                    }
                    hex += shift_vec;
                }
            }
            if all_outside_radius {
                break;
            }
            i += 1;
        }
        points
    }
}

impl From<HexagonalTiling> for super::PosDistType {
    fn from(hexagonal_tiling: HexagonalTiling) -> Self {
        Self::HexagonalTiling(hexagonal_tiling)
    }
}

#[cfg(test)]
mod tests {
    use crate::{distributions::position::HexagonalTiling, error::OpmResult, millimeter};
    use approx::assert_relative_eq;

    #[test]
    fn valid_hexagonal_tiling_creation() -> OpmResult<()> {
        let radius = millimeter!(10.0);
        let tiling = HexagonalTiling::new(radius, 5)?;

        assert_relative_eq!(tiling.radius().value, radius.value);
        assert_eq!(tiling.nr_of_hex_along_radius(), 5);
        Ok(())
    }

    #[test]
    fn invalid_negative_radius_should_error() {
        let radius = millimeter!(-1.0);
        let result = HexagonalTiling::new(radius, 5);

        assert!(result.is_err(), "negative radius must be rejected");
    }

    #[test]
    fn invalid_infinite_radius_should_error() {
        let radius = millimeter!(f64::INFINITY);
        let result = HexagonalTiling::new(radius, 5);

        assert!(result.is_err(), "infinite radius must be rejected");
    }

    #[test]
    fn invalid_nan_radius_should_error() {
        let radius = millimeter!(f64::NAN);
        let result = HexagonalTiling::new(radius, 5);

        assert!(result.is_err(), "NaN radius must be rejected");
    }

    // --- Setter Validierung ----------------------------------------------------

    #[test]
    fn set_radius_to_valid_value_should_succeed() {
        let mut t = HexagonalTiling::default();
        assert!(t.set_radius(millimeter!(20.0)).is_ok());
        assert_relative_eq!(t.radius().value, millimeter!(20.0).value);
    }

    #[test]
    fn set_radius_to_invalid_value_should_fail() {
        let mut t = HexagonalTiling::default();
        assert!(t.set_radius(millimeter!(-5.0)).is_err());
    }
    // --- Default Values --------------------------------------------------------
    #[test]
    fn default_is_valid() {
        let t = HexagonalTiling::default();
        assert!(t.radius().is_finite());
    }
}
