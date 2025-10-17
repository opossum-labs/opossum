//! Rectangluar, low-discrepancy quasirandom distribution
use super::PositionDistribution;
use crate::{
    error::OpmResult,
    generic_validators::{AllFinite, AllNotZero, AllPositive, OnlyOneZero},
    millimeter, validated, validated_type,
};
use nalgebra::{Point2, Point3, point};
use num::Zero;
use serde::{Deserialize, Serialize};
use sobol::{Sobol, params::JoeKuoD6};
use uom::si::f64::Length;

/// Rectangluar, low-discrepancy quasirandom distribution
///
/// For further details see [here](https://en.wikipedia.org/wiki/Sobol_sequence)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Copy)]
pub struct SobolDist {
    nr_of_points: validated_type!(usize, AllNotZero),
    side_length: validated_type!(Point2<Length>, OnlyOneZero && AllFinite && AllPositive),
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
        let mut random = Self::default();
        random.set_nr_of_points(nr_of_points)?;
        random.set_side_length_x(side_length_x)?;
        random.set_side_length_y(side_length_y)?;
        Ok(random)
    }
    /// Returns the number of points in the Sobol distribution.
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

    /// Sets the number of points in the Sobol distribution.
    ///
    /// # Parameters
    ///
    /// * `nr_of_points` - The new number of points as a `usize`.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current number of points.
    ///
    /// # Errors
    /// Returns an error if validation fails
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
    /// Overwrites the current side length in the X direction.
    ///
    /// # Errors
    /// Returns an error if validation fails
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
    /// Overwrites the current side length in the Y direction.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_side_length_y(&mut self, side_length_y: Length) -> OpmResult<()> {
        self.side_length
            .set(Point2::new(self.side_length_x(), side_length_y))?;
        Ok(())
    }
}

impl Default for SobolDist {
    fn default() -> Self {
        Self {
            nr_of_points: validated!(1000_usize, AllNotZero).unwrap(),
            side_length: validated!(millimeter!(5., 5.), OnlyOneZero && AllFinite && AllPositive)
                .unwrap(),
        }
    }
}

impl PositionDistribution for SobolDist {
    fn generate(&self) -> Vec<nalgebra::Point3<Length>> {
        let nr_of_points = *self.nr_of_points.get();
        let mut points: Vec<Point3<Length>> = Vec::with_capacity(nr_of_points);
        let params = JoeKuoD6::minimal();
        let seq = Sobol::<f64>::new(2, &params);
        for point in seq.take(nr_of_points) {
            let point_x = self.side_length_x() * (point[0] - 0.5);
            let point_y = self.side_length_y() * (point[1] - 0.5);
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
