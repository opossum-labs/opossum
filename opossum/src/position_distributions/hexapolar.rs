//! Circular, hexapolar distribution
use crate::{
    error::{OpmResult, OpossumError},
    millimeter,
};

use super::PositionDistribution;
use nalgebra::{Point3, point};
use num::Zero;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

/// Circular, hexapolar distribution
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Copy)]
pub struct Hexapolar {
    nr_of_rings: u8,
    radius: Length,
}
impl Hexapolar {
    /// Create a new [`Hexapolar`] distribution generator.
    ///
    /// If the given radius is zero and / or `nr_of_rings` is zero only the central point at (0,0) is generated.
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

    pub fn radius(&self) -> Length {
        self.radius
    }

    pub fn nr_of_rings(&self) -> u8 {
        self.nr_of_rings
    }

    pub fn set_radius(&mut self, radius: Length) {
        self.radius = radius;
    }

    pub fn set_nr_of_rings(&mut self, nr_of_rings: u8) {
        self.nr_of_rings = nr_of_rings;
    }
}

impl Default for Hexapolar {
    fn default() -> Self {
        Self {
            nr_of_rings: 7,
            radius: millimeter!(5.),
        }
    }
}

impl PositionDistribution for Hexapolar {
    fn generate(&self) -> Vec<Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::new();
        // Add center point
        points.push(Point3::origin());
        // add rings if radius > 0.0
        if !self.radius.is_zero() {
            let radius_step = self.radius / f64::from(self.nr_of_rings);
            for ring in 0..self.nr_of_rings {
                let radius = f64::from(ring + 1) * radius_step;
                let points_per_ring = 6 * u16::from(ring + 1);
                let angle_step = 2.0 * std::f64::consts::PI / f64::from(points_per_ring);
                for point_nr in 0..points_per_ring {
                    let point = (f64::from(point_nr) * angle_step).sin_cos();
                    points.push(point![radius * point.0, radius * point.1, Length::zero()]);
                }
            }
        }
        points
    }
}
impl From<Hexapolar> for super::PosDistType {
    fn from(dist: Hexapolar) -> Self {
        Self::Hexapolar(dist)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::millimeter;
    use std::u8;
    #[test]
    fn new_wrong() {
        assert!(Hexapolar::new(millimeter!(-0.1), 1).is_err());
        assert!(Hexapolar::new(millimeter!(f64::NAN), 1).is_err());
        assert!(Hexapolar::new(millimeter!(f64::INFINITY), 1).is_err());
    }
    #[test]
    fn generate_one() {
        let g = Hexapolar::new(Length::zero(), 1).unwrap();
        assert_eq!(g.generate().len(), 1);
        let g = Hexapolar::new(millimeter!(1.0), 0).unwrap();
        assert_eq!(g.generate().len(), 1);
    }
    #[test]
    fn generate() {
        let g = Hexapolar::new(millimeter!(1.0), 1).unwrap();
        assert_eq!(g.generate().len(), 7);
        let g = Hexapolar::new(millimeter!(1.0), 2).unwrap();
        assert_eq!(g.generate().len(), 19);
    }
    #[test]
    fn generate_max() {
        let g = Hexapolar::new(millimeter!(1.0), u8::MAX).unwrap();
        assert_eq!(g.generate().len(), 195841);
    }
    #[test]
    fn generate_rounding() {
        let g = Hexapolar::new(millimeter!(1.0), 6).unwrap();
        assert_eq!(g.generate().len(), 127);
        let g = Hexapolar::new(millimeter!(1.0), 7).unwrap();
        assert_eq!(g.generate().len(), 169);
    }
}
