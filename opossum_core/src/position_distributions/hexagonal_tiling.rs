//! Circular, hexapolar distribution
use std::f64::consts::PI;

use crate::{
    error::OpmResult,
    generic_validators::{AllFinite, AllPositive},
    meter, millimeter, validated, validated_type,
};

use super::PositionDistribution;
use nalgebra::{Point2, Point3, Vector3};
use num::{ToPrimitive, Zero};
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

/// Circular, hexapolar distribution
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Copy)]
pub struct HexagonalTiling {
    nr_of_hex_along_radius: u8,
    radius: validated_type!(Length, AllPositive && AllFinite ),
    center: validated_type!(Point2<Length>, AllFinite),
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
    pub fn new(
        radius: Length,
        nr_of_hex_along_radius: u8,
        center: Point2<Length>,
    ) -> OpmResult<Self> {
        let mut hexagonal = Self::default();
        hexagonal.set_radius(radius)?;
        hexagonal.set_center_x(center.x)?;
        hexagonal.set_center_y(center.y)?;
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

    /// Returns the center point of the hexagonal tiling.
    ///
    /// # Returns
    ///
    /// The center as a `Point2<Length>`.
    #[must_use]
    pub fn center(&self) -> Point2<Length> {
        *self.center.get()
    }

    /// Returns the x coordinate of center point of the hexagonal tiling.
    ///
    /// # Returns
    ///
    /// The center x as `Length`.
    #[must_use]
    pub fn center_x(&self) -> Length {
        self.center.get().x
    }

    /// Returns the y coordinate of center point of the hexagonal tiling.
    ///
    /// # Returns
    ///
    /// The center y as `Length`.
    #[must_use]
    pub fn center_y(&self) -> Length {
        self.center.get().y
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

    /// Sets the center point of the hexagonal tiling.
    ///
    /// # Parameters
    ///
    /// * `center` - The new center as a `Point2<Length>`.
    ///
    /// # Side Effects
    ///
    /// Updates the current center point.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_center(&mut self, center: Point2<Length>) -> OpmResult<()> {
        self.center.set(center)?;
        Ok(())
    }

    /// Sets the X coordinate of the center point.
    ///
    /// # Parameters
    ///
    /// * `center_x` - The new X coordinate as a `Length`.
    ///
    /// # Side Effects
    ///
    /// Updates the X coordinate of the center.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_center_x(&mut self, center_x: Length) -> OpmResult<()> {
        self.center.set(Point2::new(center_x, self.center_y()))?;
        Ok(())
    }

    /// Sets the Y coordinate of the center point.
    ///
    /// # Parameters
    ///
    /// * `center_y` - The new Y coordinate as a `Length`.
    ///
    /// # Side Effects
    ///
    /// Updates the Y coordinate of the center.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_center_y(&mut self, center_y: Length) -> OpmResult<()> {
        self.center.set(Point2::new(self.center_x(), center_y))?;
        Ok(())
    }
}

impl Default for HexagonalTiling {
    fn default() -> Self {
        Self {
            nr_of_hex_along_radius: 7,
            radius: validated!(millimeter!(5.), AllPositive && AllFinite).unwrap(),
            center: validated!(millimeter!(0., 0.), AllFinite).unwrap(),
        }
    }
}

impl PositionDistribution for HexagonalTiling {
    fn generate(&self) -> Vec<Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::new();
        // Add center point
        points.push(Point3::<Length>::new(
            self.center_x(),
            self.center_y(),
            meter!(0.),
        ));

        let radius_step = *self.radius.get() / self.nr_of_hex_along_radius.to_f64().unwrap();
        let mut i = 1;
        let border_radius = *self.radius.get() * 5.0f64.mul_add(f64::EPSILON, 1.);
        loop {
            let mut all_outside_radius = true;
            let mut hex = Point3::<Length>::new(self.center_x(), self.center_y(), meter!(0.));
            hex.x = radius_step * i.to_f64().unwrap() + self.center_x();
            for j in 0_u8..6 {
                let angle = PI / 3. * (2. + j.to_f64().unwrap());
                let shift_vec = Vector3::new(
                    f64::cos(angle) * radius_step,
                    f64::sin(angle) * radius_step,
                    Length::zero(),
                );
                for _k in 0_u8..i {
                    if ((hex.x - self.center_x()) * (hex.x - self.center_x())
                        + (hex.y - self.center_y()) * (hex.y - self.center_y()))
                    .sqrt()
                        <= border_radius
                    {
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
    use approx::assert_relative_eq;

    use crate::{millimeter, position_distributions::HexagonalTiling};

    #[test]
    fn valid_hexagonal_tiling_creation() {
        let radius = millimeter!(10.0);
        let center = millimeter!(0.0, 0.0);
        let tiling = HexagonalTiling::new(radius, 5, center).unwrap();

        assert_relative_eq!(tiling.radius().value, radius.value);
        assert_eq!(tiling.nr_of_hex_along_radius(), 5);
        assert_relative_eq!(tiling.center().x.value, millimeter!(0.0).value);
        assert_relative_eq!(tiling.center().y.value, millimeter!(0.0).value);
    }

    #[test]
    fn invalid_negative_radius_should_error() {
        let radius = millimeter!(-1.0);
        let center = millimeter!(0.0, 0.0);
        let result = HexagonalTiling::new(radius, 5, center);

        assert!(result.is_err(), "negative radius must be rejected");
    }

    #[test]
    fn invalid_infinite_radius_should_error() {
        let radius = millimeter!(f64::INFINITY);
        let center = millimeter!(0.0, 0.0);
        let result = HexagonalTiling::new(radius, 5, center);

        assert!(result.is_err(), "infinite radius must be rejected");
    }

    #[test]
    fn invalid_nan_radius_should_error() {
        let radius = millimeter!(f64::NAN);
        let center = millimeter!(0.0, 0.0);
        let result = HexagonalTiling::new(radius, 5, center);

        assert!(result.is_err(), "NaN radius must be rejected");
    }

    #[test]
    fn invalid_center_nan_should_error() {
        let radius = millimeter!(5.0);
        let center = millimeter!(f64::NAN, 0.0);
        let result = HexagonalTiling::new(radius, 5, center);

        assert!(result.is_err(), "NaN in center must be rejected");
    }

    #[test]
    fn invalid_center_infinite_should_error() {
        let radius = millimeter!(5.0);
        let center = millimeter!(f64::INFINITY, 0.0);
        let result = HexagonalTiling::new(radius, 5, center);

        assert!(
            result.is_err(),
            "infinite center coordinate must be rejected"
        );
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

    #[test]
    fn set_center_to_valid_value_should_succeed() {
        let mut t = HexagonalTiling::default();
        assert!(t.set_center(millimeter!(2.0, 3.0)).is_ok());
        assert_relative_eq!(t.center().x.value, millimeter!(2.0).value);
        assert_relative_eq!(t.center().y.value, millimeter!(3.0).value);
    }

    #[test]
    fn set_center_to_invalid_value_should_fail() {
        let mut t = HexagonalTiling::default();
        assert!(t.set_center(millimeter!(f64::NAN, 0.0)).is_err());
    }

    #[test]
    fn set_center_x_to_invalid_value_should_fail() {
        let mut t = HexagonalTiling::default();
        assert!(t.set_center_x(millimeter!(f64::INFINITY)).is_err());
    }

    #[test]
    fn set_center_y_to_invalid_value_should_fail() {
        let mut t = HexagonalTiling::default();
        assert!(t.set_center_y(millimeter!(f64::NAN)).is_err());
    }

    // --- Default Values --------------------------------------------------------

    #[test]
    fn getters_are_same() {
        let t = HexagonalTiling::default();
        assert_relative_eq!(t.center_x().value, t.center().x.value);
        assert_relative_eq!(t.center_y().value, t.center().y.value);
    }

    #[test]
    fn default_is_valid() {
        let t = HexagonalTiling::default();
        assert!(t.radius().is_finite());
        assert!(t.center_x().is_finite());
        assert!(t.center_y().is_finite());
    }
}
