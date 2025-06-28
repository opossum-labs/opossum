#![warn(missing_docs)]
//! Rectangular, evenly-sized grid distribution
use super::PositionDistribution;
use crate::{
    error::{OpmResult, OpossumError},
    millimeter,
    utils::usize_to_f64,
};
use nalgebra::Point3;
use num::Zero;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

/// Rectangular, evenly-sized grid distribution
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Copy)]
pub struct Grid {
    nr_of_points: (usize, usize),
    side_length: (Length, Length),
}

impl Grid {
    /// Create a new [`Grid`] distribution generator.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///  - both side lengths are zero.
    ///  - one `side_length` components is negative or not finite.
    ///  - one `nr_of_points` components is zero.
    pub fn new(side_length: (Length, Length), nr_of_points: (usize, usize)) -> OpmResult<Self> {
        if side_length.0.is_zero() && side_length.1.is_zero() {
            return Err(OpossumError::Other(
                "at least one side length must be > zero".into(),
            ));
        }
        if side_length.0.is_sign_negative() || !side_length.0.is_finite() {
            return Err(OpossumError::Other(
                "side length x must be >= zero and finite".into(),
            ));
        }
        if side_length.1.is_sign_negative() || !side_length.1.is_finite() {
            return Err(OpossumError::Other(
                "side length x must be >= zero and finite".into(),
            ));
        }
        if nr_of_points.0.is_zero() || nr_of_points.1.is_zero() {
            return Err(OpossumError::Other(
                "both components of nr_of_points must be > 0".into(),
            ));
        }
        Ok(Self {
            nr_of_points,
            side_length,
        })
    }

    /// Returns the number of points along the X and Y axes.
    ///
    /// # Returns
    ///
    /// A tuple `(usize, usize)` where the first element is the number of points in the X direction
    /// and the second element is the number of points in the Y direction.
    #[must_use]
    pub const fn nr_of_points(&self) -> (usize, usize) {
        self.nr_of_points
    }

    /// Returns the side lengths along the X and Y axes.
    ///
    /// # Returns
    ///
    /// A tuple `(Length, Length)` representing the lengths in the X and Y directions.
    #[must_use]
    pub fn side_length(&self) -> (Length, Length) {
        self.side_length
    }

    /// Returns the side length along the X axis.
    ///
    /// # Returns
    ///
    /// The length in the X direction of type `Length`.
    #[must_use]
    pub fn side_length_x(&self) -> Length {
        self.side_length.0
    }

    /// Returns the side length along the Y axis.
    ///
    /// # Returns
    ///
    /// The length in the Y direction of type `Length`.
    #[must_use]
    pub fn side_length_y(&self) -> Length {
        self.side_length.1
    }

    /// Sets the number of points along the X and Y axes.
    ///
    /// # Parameters
    ///
    /// * `nr_of_points` - A tuple `(usize, usize)` specifying the new number of points in X and Y directions.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current number of points.
    pub const fn set_nr_of_points(&mut self, nr_of_points: (usize, usize)) {
        self.nr_of_points = nr_of_points;
    }

    /// Sets the number of points along the X axis.
    ///
    /// # Parameters
    ///
    /// * `nr_of_points_x` - The new number of points in the X direction.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current X direction points count.
    pub const fn set_nr_of_points_x(&mut self, nr_of_points_x: usize) {
        self.nr_of_points.0 = nr_of_points_x;
    }

    /// Sets the number of points along the Y axis.
    ///
    /// # Parameters
    ///
    /// * `nr_of_points_y` - The new number of points in the Y direction.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current Y direction points count.
    pub const fn set_nr_of_points_y(&mut self, nr_of_points_y: usize) {
        self.nr_of_points.1 = nr_of_points_y;
    }

    /// Sets the side lengths along the X and Y axes.
    ///
    /// # Parameters
    ///
    /// * `side_length` - A tuple `(Length, Length)` specifying the new side lengths for X and Y directions.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current side lengths.
    pub fn set_side_length(&mut self, side_length: (Length, Length)) {
        self.side_length = side_length;
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
    pub fn set_side_length_x(&mut self, side_length_x: Length) {
        self.side_length.0 = side_length_x;
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
    pub fn set_side_length_y(&mut self, side_length_y: Length) {
        self.side_length.1 = side_length_y;
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self {
            nr_of_points: (100, 100),
            side_length: (millimeter!(5.), millimeter!(5.)),
        }
    }
}

impl PositionDistribution for Grid {
    fn generate(&self) -> Vec<Point3<Length>> {
        let nr_of_points_x = self.nr_of_points.0.clamp(1, usize::MAX);
        let nr_of_points_y = self.nr_of_points.1.clamp(1, usize::MAX);
        let distance_x = if nr_of_points_x > 1 {
            self.side_length.0 / usize_to_f64(nr_of_points_x - 1)
        } else {
            Length::zero()
        };
        let distance_y = if nr_of_points_y > 1 {
            self.side_length.1 / usize_to_f64(nr_of_points_y - 1)
        } else {
            Length::zero()
        };
        let offset_x = if nr_of_points_x > 1 {
            self.side_length.0 / 2.0
        } else {
            Length::zero()
        };
        let offset_y = if nr_of_points_y > 1 {
            self.side_length.1 / 2.0
        } else {
            Length::zero()
        };
        let mut points: Vec<Point3<Length>> = Vec::with_capacity(nr_of_points_x * nr_of_points_y);
        for i_x in 0..nr_of_points_x {
            for i_y in 0..nr_of_points_y {
                points.push(Point3::new(
                    usize_to_f64(i_x) * distance_x - offset_x,
                    usize_to_f64(i_y) * distance_y - offset_y,
                    Length::zero(),
                ));
            }
        }
        points
    }
}

impl From<Grid> for super::PosDistType {
    fn from(grid: Grid) -> Self {
        Self::Grid(grid)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::millimeter;
    #[test]
    fn new_wrong() {
        assert!(Grid::new((Length::zero(), Length::zero()), (1, 1)).is_err());
        assert!(Grid::new((Length::zero(), millimeter!(1.0)), (1, 1)).is_ok());
        assert!(Grid::new((millimeter!(1.0), Length::zero()), (1, 1)).is_ok());
        assert!(Grid::new((millimeter!(-0.1), millimeter!(1.0)), (1, 1)).is_err());
        assert!(Grid::new((millimeter!(f64::NAN), millimeter!(1.0)), (1, 1)).is_err());
        assert!(Grid::new((millimeter!(f64::INFINITY), millimeter!(1.0)), (1, 1)).is_err());

        assert!(Grid::new((millimeter!(1.0), millimeter!(-0.1)), (1, 1)).is_err());
        assert!(Grid::new((millimeter!(1.0), millimeter!(f64::NAN)), (1, 1)).is_err());
        assert!(Grid::new((millimeter!(1.0), millimeter!(f64::INFINITY)), (1, 1)).is_err());
        assert!(Grid::new((millimeter!(1.0), millimeter!(1.0)), (0, 1)).is_err());
        assert!(Grid::new((millimeter!(1.0), millimeter!(1.0)), (1, 0)).is_err());
    }
    #[test]
    fn generate_symmetric() {
        let strategy = Grid::new((millimeter!(1.0), millimeter!(1.0)), (2, 2)).unwrap();
        let points = strategy.generate();
        assert_eq!(points.len(), 4);
        assert_eq!(points[0], millimeter!(-0.5, -0.5, 0.));
        assert_eq!(points[1], millimeter!(-0.5, 0.5, 0.));
        assert_eq!(points[2], millimeter!(0.5, -0.5, 0.));
        assert_eq!(points[3], millimeter!(0.5, 0.5, 0.));
    }
    #[test]
    fn generate_size_one() {
        let strategy = Grid::new((millimeter!(1.0), millimeter!(1.0)), (1, 1)).unwrap();
        let points = strategy.generate();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0], millimeter!(0., 0., 0.));
    }
    #[test]
    fn generate_asymmetric() {
        let strategy = Grid::new((millimeter!(1.0), millimeter!(1.0)), (1, 2)).unwrap();
        let points = strategy.generate();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0], millimeter!(0., -0.5, 0.));
        assert_eq!(points[1], millimeter!(0., 0.5, 0.));
    }
}
