//! Circular, hexapolar distribution
use std::f64::consts::PI;

use crate::{
    error::{OpmResult, OpossumError},
    meter, millimeter,
};

use super::PositionDistribution;
use nalgebra::{Point2, Point3, Vector3};
use num::{ToPrimitive, Zero};
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

/// Circular, hexapolar distribution
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HexagonalTiling {
    nr_of_hex_along_radius: u8,
    radius: Length,
    center: Point2<Length>,
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
        if radius.is_sign_negative() || !radius.is_finite() {
            return Err(OpossumError::Other(
                "radius must be positive and finite".into(),
            ));
        }
        if !center.x.is_finite() || !center.y.is_finite() {
            return Err(OpossumError::Other(
                "center coordinates must be finite".into(),
            ));
        }
        Ok(Self {
            nr_of_hex_along_radius,
            radius,
            center,
        })
    }
}

impl Default for HexagonalTiling{
    fn default() -> Self {
        Self {
            nr_of_hex_along_radius: 7,
            radius: millimeter!(5.),
            center: millimeter!(0.,0.),
        }
    }
}

impl PositionDistribution for HexagonalTiling {
    fn generate(&self) -> Vec<Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::new();
        // Add center point
        points.push(Point3::<Length>::new(
            self.center.x,
            self.center.y,
            meter!(0.),
        ));

        let radius_step = self.radius / self.nr_of_hex_along_radius.to_f64().unwrap();
        let mut i = 1;
        let border_radius = self.radius * 5.0f64.mul_add(f64::EPSILON, 1.);
        loop {
            let mut all_outside_radius = true;
            let mut hex = Point3::<Length>::new(self.center.x, self.center.y, meter!(0.));
            hex.x = radius_step * i.to_f64().unwrap() + self.center.x;
            for j in 0_u8..6 {
                let angle = PI / 3. * (2. + j.to_f64().unwrap());
                let shift_vec = Vector3::new(
                    f64::cos(angle) * radius_step,
                    f64::sin(angle) * radius_step,
                    Length::zero(),
                );
                for _k in 0_u8..i {
                    if ((hex.x - self.center.x) * (hex.x - self.center.x)
                        + (hex.y - self.center.y) * (hex.y - self.center.y))
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
