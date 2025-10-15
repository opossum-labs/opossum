#![warn(missing_docs)]
//! Rectangular, uniform random distribution
use super::PositionDistribution;
use crate::{
    error::OpmResult,
    generic_validators::{IsFinite, IsNotZero, IsPositive, OnlyOneZero},
    millimeter, validated, validated_type,
};
use nalgebra::{Point2, Point3, point};
use num::Zero;
use rand::Rng;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

/// Rectangular, uniform random distribution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]
pub struct Random {
    nr_of_points: validated_type!(usize, IsNotZero),
    side_length: validated_type!(Point2<Length>, OnlyOneZero && IsFinite && IsPositive),
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
        let mut random = Self::default();
        random.set_nr_of_points(nr_of_points)?;
        random.set_side_length_x(side_length_x)?;
        random.set_side_length_y(side_length_y)?;
        Ok(random)
    }

    /// Returns the number of points in the random distribution.
    ///
    /// # Returns
    ///
    /// The number of points as a `usize`.
    #[must_use]
    pub const fn nr_of_points(&self) -> usize {
        *self.nr_of_points.get()
    }

    /// Returns the side length along the X axis.
    ///
    /// # Returns
    ///
    /// The side length in the X direction of type `Length`.
    #[must_use]
    pub fn side_length_x(&self) -> Length {
        self.side_length.get().x
    }

    /// Returns the side length along the Y axis.
    ///
    /// # Returns
    ///
    /// The side length in the Y direction of type `Length`.
    #[must_use]
    pub fn side_length_y(&self) -> Length {
        self.side_length.get().y
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
    ///
    /// # Errors
    /// Returns an error if validation of the passed value fails
    pub fn set_nr_of_points(&mut self, nr_of_points: usize) -> OpmResult<()> {
        self.nr_of_points.set(nr_of_points)?;
        Ok(())
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
    ///
    /// # Errors
    /// Returns an error if validation of the passed value fails
    pub fn set_side_length_x(&mut self, side_length_x: Length) -> OpmResult<()> {
        self.side_length
            .set(Point2::new(side_length_x, self.side_length_y()))?;
        Ok(())
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
    ///
    /// # Errors
    /// Returns an error if validation of the passed value fails
    pub fn set_side_length_y(&mut self, side_length_y: Length) -> OpmResult<()> {
        self.side_length
            .set(Point2::new(self.side_length_x(), side_length_y))?;
        Ok(())
    }
}

impl Default for Random {
    fn default() -> Self {
        Self {
            nr_of_points: validated!(1000_usize, IsNotZero).unwrap(),
            side_length: validated!(millimeter!(5., 5.), OnlyOneZero && IsFinite && IsPositive)
                .unwrap(),
        }
    }
}

impl PositionDistribution for Random {
    fn generate(&self) -> Vec<nalgebra::Point3<Length>> {
        let nr_of_points = *self.nr_of_points.get();
        let mut points: Vec<Point3<Length>> = Vec::with_capacity(nr_of_points);
        let mut rng = rand::rng();
        for _ in 0..nr_of_points {
            let point_x = self.side_length_x() * rng.random_range(-1.0..1.0);
            let point_y = self.side_length_y() * rng.random_range(-1.0..1.0);
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
    fn new_ok() {
        assert!(Random::new(millimeter!(1.0), Length::zero(), 1).is_ok());
        assert!(Random::new(Length::zero(), millimeter!(1.0), 1).is_ok());
        assert!(Random::new(millimeter!(1.), millimeter!(1.0), 1).is_ok());
    }
    #[test]
    fn set_ok() {
        let mut random = Random::new(millimeter!(1.0), Length::zero(), 1).unwrap();

        assert!(random.set_nr_of_points(10).is_ok());
        assert!(random.set_nr_of_points(100).is_ok());
        assert!(random.set_side_length_x(millimeter!(10.)).is_ok());
        assert!(random.set_side_length_y(millimeter!(10.)).is_ok());
        assert!(random.set_side_length_x(millimeter!(0.)).is_ok());
    }
    #[test]
    fn set_err() {
        let mut random = Random::new(millimeter!(1.0), Length::zero(), 1).unwrap();

        assert!(random.set_nr_of_points(0).is_err());
        assert!(random.set_side_length_x(millimeter!(-10.)).is_err());
        assert!(random.set_side_length_y(millimeter!(-10.)).is_err());
        assert!(random.set_side_length_x(millimeter!(0.)).is_err());

        let mut random = Random::new(Length::zero(), millimeter!(1.0), 1).unwrap();
        assert!(random.set_nr_of_points(0).is_err());
        assert!(random.set_side_length_x(millimeter!(-10.)).is_err());
        assert!(random.set_side_length_y(millimeter!(-10.)).is_err());
        assert!(random.set_side_length_y(millimeter!(0.)).is_err());
    }
    #[test]
    fn generate() {
        let strategy = Random::new(millimeter!(1.0), millimeter!(1.0), 10).unwrap();
        assert_eq!(strategy.generate().len(), 10);
    }
}
