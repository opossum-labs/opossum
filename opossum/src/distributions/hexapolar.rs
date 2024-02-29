//! Circular, hexapolar distribution
use crate::error::{OpmResult, OpossumError};

use super::Distribution;
use nalgebra::{point, Point3};
use num::Zero;
use uom::si::f64::Length;

pub struct Hexapolar {
    nr_of_rings: u8,
    radius: Length,
}
impl Hexapolar {
    /// Create a new [`Hexaploar`] distribution generator.
    ///
    /// If the given radius is zero only the central point at (0,0) is generated.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///  - the given `radius` is negative or not finite.
    pub fn new(radius: Length, nr_of_rings: u8) -> OpmResult<Self> {
        if radius.is_sign_negative() || !radius.is_finite() {
            return Err(OpossumError::Other(
                "radius must be positive and finite".into(),
            ));
        }
        Ok(Self {
            nr_of_rings,
            radius,
        })
    }
}
impl Distribution for Hexapolar {
    fn generate(&self) -> Vec<Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::new();
        // Add center point
        points.push(Point3::origin());
        // add rings if radius > 0.0
        if !self.radius.is_zero() {
            let radius_step = self.radius / f64::from(self.nr_of_rings);
            for ring in 0u8..self.nr_of_rings {
                let radius = f64::from(ring + 1) * radius_step;
                let points_per_ring = 6 * (ring + 1);
                let angle_step = 2.0 * std::f64::consts::PI / f64::from(points_per_ring);
                for point_nr in 0u8..points_per_ring {
                    let point = (f64::from(point_nr) * angle_step).sin_cos();
                    points.push(point![radius * point.0, radius * point.1, Length::zero()]);
                }
            }
        }
        points
    }
}
