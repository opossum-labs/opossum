#![warn(missing_docs)]
//! Rectangular, uniform random distribution
use super::PositionDistribution;
use crate::{
    error::{OpmResult, OpossumError},
    millimeter,
};
use nalgebra::{Point3, point};
use num::Zero;
use rand::Rng;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

/// Rectangular, uniform random distribution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]
pub struct Random {
    nr_of_points: usize,
    side_length_x: Length,
    side_length_y: Length,
}
impl Random {
    /// Create a new [`Random`] distribution generator.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - both side lengths are zero.
    ///   - a side length must be >= zero and finite.
    ///   - `nr_of_points` must be >= 1.
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
        if side_length_x.is_sign_negative() || !side_length_x.is_normal() {
            return Err(OpossumError::Other(
                "side_length_x must be >= zero and finite".into(),
            ));
        }
        if side_length_y.is_sign_negative() || !side_length_y.is_normal() {
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

    /// Returns the number of points in the random distribution.
    ///
    /// # Returns
    ///
    /// The number of points as a `usize`.
    pub fn nr_of_points(&self) -> usize {
        self.nr_of_points
    }

    /// Returns the side length along the X axis.
    ///
    /// # Returns
    ///
    /// The side length in the X direction of type `Length`.
    pub fn side_length_x(&self) -> Length {
        self.side_length_x
    }

    /// Returns the side length along the Y axis.
    ///
    /// # Returns
    ///
    /// The side length in the Y direction of type `Length`.
    pub fn side_length_y(&self) -> Length {
        self.side_length_y
    }

    /// Sets the number of points in the random distribution.
    ///
    /// # Parameters
    ///
    /// * `nr_of_points` - The new number of points as a `usize`.
    ///
    /// # Side Effects
    ///
    /// Updates the current number of points.
    pub fn set_nr_of_points(&mut self, nr_of_points: usize) {
        self.nr_of_points = nr_of_points;
    }

    /// Sets the side length along the X axis.
    ///
    /// # Parameters
    ///
    /// * `side_length_x` - The new side length in the X direction.
    ///
    /// # Side Effects
    ///
    /// Updates the current side length in the X direction.
    pub fn set_side_length_x(&mut self, side_length_x: Length) {
        self.side_length_x = side_length_x;
    }

    /// Sets the side length along the Y axis.
    ///
    /// # Parameters
    ///
    /// * `side_length_y` - The new side length in the Y direction.
    ///
    /// # Side Effects
    ///
    /// Updates the current side length in the Y direction.
    pub fn set_side_length_y(&mut self, side_length_y: Length) {
        self.side_length_y = side_length_y;
    }
}

impl Default for Random {
    fn default() -> Self {
        Self {
            nr_of_points: 1000,
            side_length_x: millimeter!(5.),
            side_length_y: millimeter!(5.),
        }
    }
}

impl PositionDistribution for Random {
    fn generate(&self) -> Vec<nalgebra::Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::with_capacity(self.nr_of_points);
        let mut rng = rand::rng();
        for _ in 0..self.nr_of_points {
            let point_x = self.side_length_x * rng.random_range(-1.0..1.0);
            let point_y = self.side_length_y * rng.random_range(-1.0..1.0);
            points.push(point![point_x, point_y, Length::zero()]);
        }
        points
    }
}
impl From<Random> for super::PosDistType {
    fn from(random: Random) -> Self {
        Self::Random(random)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::millimeter;
    #[test]
    fn new_wrong() {
        assert!(Random::new(Length::zero(), Length::zero(), 1).is_err());
        assert!(Random::new(millimeter!(-0.1), millimeter!(1.0), 1).is_err());
        assert!(Random::new(millimeter!(f64::NAN), millimeter!(1.0), 1).is_err());
        assert!(Random::new(millimeter!(f64::INFINITY), millimeter!(1.0), 1).is_err());

        assert!(Random::new(millimeter!(1.0), millimeter!(-0.1), 1).is_err());
        assert!(Random::new(millimeter!(1.0), millimeter!(f64::NAN), 1).is_err());
        assert!(Random::new(millimeter!(1.0), millimeter!(f64::INFINITY), 1).is_err());
        assert!(Random::new(millimeter!(1.0), millimeter!(1.0), 0).is_err());
    }
    #[test]
    fn generate() {
        let strategy = Random::new(millimeter!(1.0), millimeter!(1.0), 10).unwrap();
        assert_eq!(strategy.generate().len(), 10);
    }
}
