#![warn(missing_docs)]
//! Rectangular, evenly-sized grid distribution
use super::PositionDistribution;
use crate::error::{OpmResult, OpossumError};
use nalgebra::Point3;
use num::Zero;
use uom::si::f64::Length;

/// Rectangular, evenly-sized grid distribution
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
}

impl PositionDistribution for Grid {
    fn generate(&self) -> Vec<Point3<Length>> {
        let nr_of_points_x = self.nr_of_points.0.clamp(1, usize::MAX);
        let nr_of_points_y = self.nr_of_points.1.clamp(1, usize::MAX);
        #[allow(clippy::cast_precision_loss)]
        let distance_x = if nr_of_points_x > 1 {
            self.side_length.0 / ((nr_of_points_x - 1) as f64)
        } else {
            Length::zero()
        };
        #[allow(clippy::cast_precision_loss)]
        let distance_y = if nr_of_points_y > 1 {
            self.side_length.1 / ((nr_of_points_y - 1) as f64)
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
                #[allow(clippy::cast_precision_loss)]
                points.push(Point3::new(
                    (i_x as f64) * distance_x - offset_x,
                    (i_y as f64) * distance_y - offset_y,
                    Length::zero(),
                ));
            }
        }
        points
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
