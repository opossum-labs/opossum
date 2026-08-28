#![warn(missing_docs)]
//! Rectangular, evenly-sized grid distribution
use super::PositionDistribution;
use crate::{
    error::OpmResult,
    generic_validators::{AllFinite, AllNotZero, AllPositive, NotAllZero},
    millimeter,
    utils::to_f64,
    validated, validated_type,
};
use nalgebra::{Point2, Point3};
use num::Zero;
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

/// Rectangular, evenly-sized grid distribution
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Copy, EnsureValidated)]
pub struct Grid {
    nr_of_points: validated_type!(Point2<usize>, AllNotZero),
    side_length: validated_type!(Point2<Length>, NotAllZero && AllFinite && AllPositive),
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
    pub fn new(side_length: Point2<Length>, nr_of_points: Point2<usize>) -> OpmResult<Self> {
        let mut grid = Self::default();
        grid.set_nr_of_points_x(nr_of_points.x)?;
        grid.set_nr_of_points_y(nr_of_points.y)?;
        grid.set_side_length_x(side_length.x)?;
        grid.set_side_length_y(side_length.y)?;

        Ok(grid)
    }

    /// Returns the number of points along the X and Y axes.
    ///
    /// # Returns
    ///
    /// A tuple `Point2<usize>` where the first element is the number of points in the X direction
    /// and the second element is the number of points in the Y direction.
    #[must_use]
    pub const fn nr_of_points(&self) -> Point2<usize> {
        *self.nr_of_points.get()
    }

    /// Returns the number of points along the X axis.
    ///
    /// # Returns
    ///
    /// the number of points in the X direction.
    #[must_use]
    pub fn nr_of_points_x(&self) -> usize {
        self.nr_of_points.get().x
    }

    /// Returns the number of points along the Y axis.
    ///
    /// # Returns
    ///
    /// the number of points in the Y direction.
    #[must_use]
    pub fn nr_of_points_y(&self) -> usize {
        self.nr_of_points.get().y
    }

    /// Returns the side lengths along the X and Y axes.
    ///
    /// # Returns
    ///
    /// A tuple `Point2<Length>` representing the lengths in the X and Y directions.
    #[must_use]
    pub const fn side_length(&self) -> Point2<Length> {
        *self.side_length.get()
    }

    /// Returns the side length along the X axis.
    ///
    /// # Returns
    ///
    /// The length in the X direction of type `Length`.
    #[must_use]
    pub fn side_length_x(&self) -> Length {
        self.side_length.get().x
    }

    /// Returns the side length along the Y axis.
    ///
    /// # Returns
    ///
    /// The length in the Y direction of type `Length`.
    #[must_use]
    pub fn side_length_y(&self) -> Length {
        self.side_length.get().y
    }

    /// Sets the number of points along the X and Y axes.
    ///
    /// # Parameters
    ///
    /// * `nr_of_points` - A `Point2<usize>` specifying the new number of points in X and Y directions.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current number of points.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_nr_of_points(&mut self, nr_of_points: Point2<usize>) -> OpmResult<()> {
        self.nr_of_points.set(nr_of_points)?;
        Ok(())
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
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_nr_of_points_x(&mut self, nr_of_points_x: usize) -> OpmResult<()> {
        self.nr_of_points
            .set(Point2::new(nr_of_points_x, self.nr_of_points_y()))?;
        Ok(())
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
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_nr_of_points_y(&mut self, nr_of_points_y: usize) -> OpmResult<()> {
        self.nr_of_points
            .set(Point2::new(self.nr_of_points_x(), nr_of_points_y))?;
        Ok(())
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
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_side_length(&mut self, side_length: Point2<Length>) -> OpmResult<()> {
        self.side_length.set(side_length)?;
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

impl Default for Grid {
    fn default() -> Self {
        Self {
            nr_of_points: validated!(Point2::new(100_usize, 100_usize), AllNotZero).unwrap(),
            side_length: validated!(millimeter!(5., 5.), NotAllZero && AllFinite && AllPositive)
                .unwrap(),
        }
    }
}

impl PositionDistribution for Grid {
    fn generate(&self) -> Vec<Point3<Length>> {
        let nr_of_points_x = self.nr_of_points_x().clamp(1, usize::MAX);
        let nr_of_points_y = self.nr_of_points_y().clamp(1, usize::MAX);
        let distance_x = if nr_of_points_x > 1 {
            self.side_length_x() / to_f64(nr_of_points_x - 1)
        } else {
            Length::zero()
        };
        let distance_y = if nr_of_points_y > 1 {
            self.side_length_y() / to_f64(nr_of_points_y - 1)
        } else {
            Length::zero()
        };
        let offset_x = if nr_of_points_x > 1 {
            self.side_length_x() / 2.0
        } else {
            Length::zero()
        };
        let offset_y = if nr_of_points_y > 1 {
            self.side_length_y() / 2.0
        } else {
            Length::zero()
        };
        let mut points: Vec<Point3<Length>> = Vec::with_capacity(nr_of_points_x * nr_of_points_y);
        for i_x in 0..nr_of_points_x {
            for i_y in 0..nr_of_points_y {
                points.push(Point3::new(
                    to_f64(i_x) * distance_x - offset_x,
                    to_f64(i_y) * distance_y - offset_y,
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
        assert!(Grid::new(millimeter!(0.0, 0.0), Point2::new(1, 1)).is_err());
        assert!(Grid::new(millimeter!(0.0, 1.0), Point2::new(1, 1)).is_ok());
        assert!(Grid::new(millimeter!(1.0, 0.0), Point2::new(1, 1)).is_ok());
        assert!(Grid::new(millimeter!(-0.1, 1.0), Point2::new(1, 1)).is_err());
        assert!(Grid::new(millimeter!(f64::NAN, 1.0), Point2::new(1, 1)).is_err());
        assert!(Grid::new(millimeter!(f64::INFINITY, 1.0), Point2::new(1, 1)).is_err());

        assert!(Grid::new(millimeter!(1.0, -0.1), Point2::new(1, 1)).is_err());
        assert!(Grid::new(millimeter!(1.0, f64::NAN), Point2::new(1, 1)).is_err());
        assert!(Grid::new(millimeter!(1.0, f64::INFINITY), Point2::new(1, 1)).is_err());
        assert!(Grid::new(millimeter!(1.0, 1.0), Point2::new(0, 1)).is_err());
        assert!(Grid::new(millimeter!(1.0, 1.0), Point2::new(1, 0)).is_err());
    }
    #[test]
    fn generate_symmetric() -> OpmResult<()> {
        let strategy = Grid::new(millimeter!(1.0, 1.0), Point2::new(2, 2))?;
        let points = strategy.generate();
        assert_eq!(points.len(), 4);
        assert_eq!(points[0], millimeter!(-0.5, -0.5, 0.));
        assert_eq!(points[1], millimeter!(-0.5, 0.5, 0.));
        assert_eq!(points[2], millimeter!(0.5, -0.5, 0.));
        assert_eq!(points[3], millimeter!(0.5, 0.5, 0.));
        Ok(())
    }
    #[test]
    fn generate_size_one() -> OpmResult<()> {
        let strategy = Grid::new(millimeter!(1.0, 1.0), Point2::new(1, 1))?;
        let points = strategy.generate();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0], millimeter!(0., 0., 0.));
        Ok(())
    }
    #[test]
    fn generate_asymmetric() -> OpmResult<()> {
        let strategy = Grid::new(millimeter!(1.0, 1.0), Point2::new(1, 2))?;
        let points = strategy.generate();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0], millimeter!(0., -0.5, 0.));
        assert_eq!(points[1], millimeter!(0., 0.5, 0.));
        Ok(())
    }
}
