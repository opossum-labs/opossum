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
    radius: validated_type!(Length, AllPositive && AllFinite),
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
