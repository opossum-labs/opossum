//! Rectangular, uniform random distribution
use super::PositionDistribution;
use crate::error::{OpmResult, OpossumError};
use nalgebra::{point, Point3};
use num::Zero;
use rand::Rng;
use uom::si::f64::Length;

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
        };
        if side_length_x.is_sign_negative() || !side_length_x.is_normal() {
            return Err(OpossumError::Other(
                "side_length_x must be >= zero and finite".into(),
            ));
        };
        if side_length_y.is_sign_negative() || !side_length_y.is_normal() {
            return Err(OpossumError::Other(
                "side_length_y must be >= zero and finite".into(),
            ));
        };
        if nr_of_points.is_zero() {
            return Err(OpossumError::Other("nr_of_points must be >= 1.".into()));
        }
        Ok(Self {
            nr_of_points,
            side_length_x,
            side_length_y,
        })
    }
}
impl PositionDistribution for Random {
    fn generate(&self) -> Vec<nalgebra::Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::new();
        let mut rng = rand::thread_rng();
        for _ in 0..self.nr_of_points {
            let point_x = self.side_length_x * rng.gen_range(-1.0..1.0);
            let point_y = self.side_length_y * rng.gen_range(-1.0..1.0);
            points.push(point![point_x, point_y, Length::zero()]);
        }
        points
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use uom::si::length::millimeter;
    #[test]
    fn new_wrong() {
        assert!(Random::new(Length::zero(), Length::zero(), 1).is_err());
        assert!(Random::new(
            Length::new::<millimeter>(-0.1),
            Length::new::<millimeter>(1.0),
            1
        )
        .is_err());
        assert!(Random::new(
            Length::new::<millimeter>(f64::NAN),
            Length::new::<millimeter>(1.0),
            1
        )
        .is_err());
        assert!(Random::new(
            Length::new::<millimeter>(f64::INFINITY),
            Length::new::<millimeter>(1.0),
            1
        )
        .is_err());

        assert!(Random::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(-0.1),
            1
        )
        .is_err());
        assert!(Random::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(f64::NAN),
            1
        )
        .is_err());
        assert!(Random::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(f64::INFINITY),
            1
        )
        .is_err());
        assert!(Random::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(1.0),
            0
        )
        .is_err());
    }
    #[test]
    fn generate() {
        let strategy = Random::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(1.0),
            10,
        )
        .unwrap();
        assert_eq!(strategy.generate().len(), 10);
    }
}
