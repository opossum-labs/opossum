//! Rectangluar, low-discrepancy quasirandom distribution
use super::PositionDistribution;
use crate::{
    error::{OpmResult, OpossumError},
    millimeter,
};
use nalgebra::{point, Point3};
use num::Zero;
use serde::{Deserialize, Serialize};
use sobol::{params::JoeKuoD6, Sobol};
use uom::si::f64::Length;

/// Rectangluar, low-discrepancy quasirandom distribution
///
/// For further details see [here](https://en.wikipedia.org/wiki/Sobol_sequence)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Copy)]
pub struct SobolDist {
    nr_of_points: usize,
    side_length_x: Length,
    side_length_y: Length,
}

impl SobolDist {
    /// Create a new [`SobolDist`] (Sobol) distribution generator.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - both side lengths are zero.
    ///   - one side length is negative or not finite
    ///   - `nr_of_points` is zero.
    pub fn new(
        side_length_x: Length,
        side_length_y: Length,
        nr_of_points: usize,
    ) -> OpmResult<Self> {
        if side_length_x.is_zero() && side_length_y.is_zero() {
            return Err(OpossumError::Other(
                "At least one side length must be != zero".into(),
            ));
        }
        if side_length_x.is_sign_negative() || !side_length_x.is_finite() {
            return Err(OpossumError::Other(
                "side_length_x must be >= zero and finite".into(),
            ));
        }
        if side_length_y.is_sign_negative() || !side_length_y.is_finite() {
            return Err(OpossumError::Other(
                "side_length_y must be >= zero and finite".into(),
            ));
        }
        if nr_of_points.is_zero() {
            return Err(OpossumError::Other("nr_of_points must be >= 1.".into()));
        }
        Ok(Self {
            nr_of_points,
            side_length_x,
            side_length_y,
        })
    }
    pub fn nr_of_points(&self) -> usize {
        self.nr_of_points
    }
    pub fn side_length_x(&self) -> Length {
        self.side_length_x
    }
    pub fn side_length_y(&self) -> Length {
        self.side_length_y
    }
    pub fn set_nr_of_points(&mut self, nr_of_points: usize) {
        self.nr_of_points = nr_of_points;
    }
    pub fn set_side_length_x(&mut self, side_length_x: Length) {
        self.side_length_x = side_length_x;
    }
    pub fn set_side_length_y(&mut self, side_length_y: Length) {
        self.side_length_y = side_length_y;
    }
}

impl Default for SobolDist {
    fn default() -> Self {
        Self {
            nr_of_points: 1000,
            side_length_x: millimeter!(5.),
            side_length_y: millimeter!(5.),
        }
    }
}

impl PositionDistribution for SobolDist {
    fn generate(&self) -> Vec<nalgebra::Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::with_capacity(self.nr_of_points);
        let params = JoeKuoD6::minimal();
        let seq = Sobol::<f64>::new(2, &params);
        for point in seq.take(self.nr_of_points) {
            let point_x = self.side_length_x * (point[0] - 0.5);
            let point_y = self.side_length_y * (point[1] - 0.5);
            points.push(point!(point_x, point_y, Length::zero()));
        }
        points
    }
}
impl From<SobolDist> for super::PosDistType {
    fn from(f: SobolDist) -> Self {
        Self::Sobol(f)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::millimeter;
    #[test]
    fn new_wrong() {
        assert!(SobolDist::new(Length::zero(), Length::zero(), 1).is_err());
        assert!(SobolDist::new(millimeter!(-0.1), millimeter!(1.0), 1).is_err());
        assert!(SobolDist::new(millimeter!(f64::NAN), millimeter!(1.0), 1).is_err());
        assert!(SobolDist::new(millimeter!(f64::INFINITY), millimeter!(1.0), 1).is_err());

        assert!(SobolDist::new(millimeter!(1.0), millimeter!(-0.1), 1).is_err());
        assert!(SobolDist::new(millimeter!(1.0), millimeter!(f64::NAN), 1).is_err());
        assert!(SobolDist::new(millimeter!(1.0), millimeter!(f64::INFINITY), 1).is_err());
        assert!(SobolDist::new(millimeter!(1.0), millimeter!(1.0), 0).is_err());
    }
    #[test]
    fn generate() {
        let strategy = SobolDist::new(millimeter!(1.0), millimeter!(1.0), 10).unwrap();
        assert_eq!(strategy.generate().len(), 10);
    }
}
