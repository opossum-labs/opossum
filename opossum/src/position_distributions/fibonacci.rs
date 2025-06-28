#![warn(missing_docs)]
//! Circular and square, fibbonacci distribution
use std::f64::consts::PI;

use crate::{
    error::{OpmResult, OpossumError},
    millimeter,
};

use super::PositionDistribution;
use nalgebra::{Point3, point};
use num::{ToPrimitive, Zero};
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

/// Rectangular Fibonacci distribution
///
/// For further details see [here](https://en.wikipedia.org/wiki/Fibonacci_sequence)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Copy)]
pub struct FibonacciRectangle {
    nr_of_rays: usize,
    side_length_x: Length,
    side_length_y: Length,
}
impl FibonacciRectangle {
    /// Create a new [`FibonacciRectangle`] distribution generator.
    ///
    /// If one of the given side lengths is zero and / or `nr_of_rays` is zero only the central point at (0,0) is generated.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///  - the given `side_length_x` or `side_length_y` is negative or not finite, or both are zero.
    ///  - the given `nr_of_rays` is zero.
    pub fn new(side_length_x: Length, side_length_y: Length, nr_of_rays: usize) -> OpmResult<Self> {
        if side_length_x.is_sign_negative()
            || !side_length_x.is_finite()
            || side_length_y.is_sign_negative()
            || !side_length_y.is_finite()
            || nr_of_rays.is_zero()
            || (side_length_x.is_zero() && side_length_y.is_zero())
        {
            return Err(OpossumError::Other(
                "side length must be positive and finite and the number of rays greater than zero!"
                    .into(),
            ));
        }
        Ok(Self {
            nr_of_rays,
            side_length_x,
            side_length_y,
        })
    }

    /// Returns the number of points (rays) in the Fibonacci rectangle distribution.
    ///
    /// # Returns
    ///
    /// The number of points as a `usize`.
    pub fn nr_of_points(&self) -> usize {
        self.nr_of_rays
    }

    /// Returns the side length along the X axis of the rectangle.
    ///
    /// # Returns
    ///
    /// The length of the side in the X direction as a `Length`.
    pub fn side_length_x(&self) -> Length {
        self.side_length_x
    }

    /// Returns the side length along the Y axis of the rectangle.
    ///
    /// # Returns
    ///
    /// The length of the side in the Y direction as a `Length`.
    pub fn side_length_y(&self) -> Length {
        self.side_length_y
    }

    /// Sets the number of points (rays) in the Fibonacci rectangle distribution.
    ///
    /// # Parameters
    ///
    /// * `nr_of_points` - The new number of points as a `usize`.
    ///
    /// # Side Effects
    ///
    /// Updates the current number of rays.
    pub fn set_nr_of_points(&mut self, nr_of_points: usize) {
        self.nr_of_rays = nr_of_points;
    }

    /// Sets the side length along the X axis of the rectangle.
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

    /// Sets the side length along the Y axis of the rectangle.
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

impl Default for FibonacciRectangle {
    fn default() -> Self {
        Self {
            nr_of_rays: 1000,
            side_length_x: millimeter!(5.),
            side_length_y: millimeter!(5.),
        }
    }
}

impl PositionDistribution for FibonacciRectangle {
    fn generate(&self) -> Vec<Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::with_capacity(self.nr_of_rays);
        let golden_ratio = f64::midpoint(1., f64::sqrt(5.));
        for i in 0_usize..self.nr_of_rays {
            let i_f64 = i.to_f64().unwrap();
            points.push(point![
                self.side_length_x * ((i_f64 / golden_ratio).fract() - 0.5),
                self.side_length_y * ((i_f64 / self.nr_of_rays.to_f64().unwrap()) - 0.5),
                Length::zero()
            ]);
        }
        points
    }
}
impl From<FibonacciRectangle> for super::PosDistType {
    fn from(f: FibonacciRectangle) -> Self {
        Self::FibonacciRectangle(f)
    }
}
/// Rectangular Fibbonacci distribution
///
/// For further details see [here](https://en.wikipedia.org/wiki/Fibonacci_sequence)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Copy)]
pub struct FibonacciEllipse {
    nr_of_rays: usize,
    radius_x: Length,
    radius_y: Length,
}
impl FibonacciEllipse {
    /// Create a new [`FibonacciEllipse`] distribution generator.
    ///
    /// If one of the given radii is zero and / or `nr_of_rays` is zero only the central point at (0,0) is generated.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///  - the given `side_length_x` or `side_length_y` is negative or not finite, or both are zero.
    ///  - the given `nr_of_rays` is zero.
    pub fn new(radius_x: Length, radius_y: Length, nr_of_rays: usize) -> OpmResult<Self> {
        if radius_x.is_sign_negative()
            || !radius_x.is_finite()
            || radius_y.is_sign_negative()
            || !radius_y.is_finite()
            || nr_of_rays.is_zero()
            || (radius_x.is_zero() && radius_y.is_zero())
        {
            return Err(OpossumError::Other(
                "radius must be positive and finite and the number of rays greater than zero!"
                    .into(),
            ));
        }
        Ok(Self {
            nr_of_rays,
            radius_x,
            radius_y,
        })
    }
    /// Returns the number of points (rays) in the Fibonacci ellipse distribution.
    ///
    /// # Returns
    ///
    /// The number of points as a `usize`.
    pub fn nr_of_points(&self) -> usize {
        self.nr_of_rays
    }

    /// Returns the radius along the X axis of the ellipse.
    ///
    /// # Returns
    ///
    /// The radius in the X direction as a `Length`.
    pub fn radius_x(&self) -> Length {
        self.radius_x
    }

    /// Returns the radius along the Y axis of the ellipse.
    ///
    /// # Returns
    ///
    /// The radius in the Y direction as a `Length`.
    pub fn radius_y(&self) -> Length {
        self.radius_y
    }

    /// Sets the number of points (rays) in the Fibonacci ellipse distribution.
    ///
    /// # Parameters
    ///
    /// * `nr_of_points` - The new number of points as a `usize`.
    ///
    /// # Side Effects
    ///
    /// Updates the current number of rays.
    pub fn set_nr_of_points(&mut self, nr_of_points: usize) {
        self.nr_of_rays = nr_of_points;
    }

    /// Sets the radius along the X axis of the ellipse.
    ///
    /// # Parameters
    ///
    /// * `radius_x` - The new radius in the X direction.
    ///
    /// # Side Effects
    ///
    /// Updates the current radius_x.
    pub fn set_radius_x(&mut self, radius_x: Length) {
        self.radius_x = radius_x;
    }

    /// Sets the radius along the Y axis of the ellipse.
    ///
    /// # Parameters
    ///
    /// * `radius_y` - The new radius in the Y direction.
    ///
    /// # Side Effects
    ///
    /// Updates the current radius_y.
    pub fn set_radius_y(&mut self, radius_y: Length) {
        self.radius_y = radius_y;
    }
}

impl Default for FibonacciEllipse {
    fn default() -> Self {
        Self {
            nr_of_rays: 1000,
            radius_x: millimeter!(5.),
            radius_y: millimeter!(5.),
        }
    }
}

impl PositionDistribution for FibonacciEllipse {
    fn generate(&self) -> Vec<Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::with_capacity(self.nr_of_rays);
        let golden_ratio = f64::midpoint(1., f64::sqrt(5.));
        for i in 0_usize..self.nr_of_rays {
            let sin_cos = f64::sin_cos(2. * PI * (i.to_f64().unwrap() / golden_ratio).fract());
            let sqrt_r = f64::sqrt(i.to_f64().unwrap() / self.nr_of_rays.to_f64().unwrap());
            points.push(point![
                self.radius_x * sin_cos.0 * sqrt_r,
                self.radius_y * sin_cos.1 * sqrt_r,
                Length::zero()
            ]);
        }
        points
    }
}

impl From<FibonacciEllipse> for super::PosDistType {
    fn from(f: FibonacciEllipse) -> Self {
        Self::FibonacciEllipse(f)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::millimeter;
    #[test]
    fn new_rect_wrong() {
        assert!(FibonacciRectangle::new(millimeter!(-0.1), millimeter!(0.1), 1).is_err());
        assert!(FibonacciRectangle::new(millimeter!(0.1), millimeter!(-0.1), 1).is_err());
        assert!(FibonacciRectangle::new(millimeter!(f64::NAN), millimeter!(0.1), 1).is_err());
        assert!(FibonacciRectangle::new(millimeter!(f64::INFINITY), millimeter!(0.1), 1).is_err());
        assert!(
            FibonacciRectangle::new(millimeter!(f64::NEG_INFINITY), millimeter!(0.1), 1).is_err()
        );
        assert!(FibonacciRectangle::new(millimeter!(0.1), millimeter!(f64::NAN), 1).is_err());
        assert!(FibonacciRectangle::new(millimeter!(0.1), millimeter!(f64::INFINITY), 1).is_err());
        assert!(
            FibonacciRectangle::new(millimeter!(0.1), millimeter!(f64::NEG_INFINITY), 1).is_err()
        );
        assert!(FibonacciRectangle::new(millimeter!(0.0), millimeter!(0.0), 1).is_err());
        assert!(FibonacciRectangle::new(Length::zero(), millimeter!(1.0), 0).is_err());
    }
    #[test]
    fn new_ellipse_wrong() {
        assert!(FibonacciEllipse::new(millimeter!(-0.1), millimeter!(0.1), 1).is_err());
        assert!(FibonacciEllipse::new(millimeter!(0.1), millimeter!(-0.1), 1).is_err());
        assert!(FibonacciEllipse::new(millimeter!(f64::NAN), millimeter!(0.1), 1).is_err());
        assert!(FibonacciEllipse::new(millimeter!(f64::INFINITY), millimeter!(0.1), 1).is_err());
        assert!(
            FibonacciEllipse::new(millimeter!(f64::NEG_INFINITY), millimeter!(0.1), 1).is_err()
        );
        assert!(FibonacciEllipse::new(millimeter!(0.1), millimeter!(f64::NAN), 1).is_err());
        assert!(FibonacciEllipse::new(millimeter!(0.1), millimeter!(f64::INFINITY), 1).is_err());
        assert!(
            FibonacciEllipse::new(millimeter!(0.1), millimeter!(f64::NEG_INFINITY), 1).is_err()
        );
        assert!(FibonacciEllipse::new(millimeter!(0.), millimeter!(0.0), 1).is_err());
        assert!(FibonacciEllipse::new(millimeter!(0.1), millimeter!(0.0), 0).is_err());
    }
    #[test]
    fn generate_one_rect() {
        assert!(FibonacciRectangle::new(Length::zero(), Length::zero(), 1).is_err());
    }
    #[test]
    fn generate_one_ellipse() {
        assert!(FibonacciEllipse::new(Length::zero(), Length::zero(), 1).is_err());
    }
    #[test]
    fn generate_rect() {
        let g = FibonacciEllipse::new(millimeter!(1.0), millimeter!(1.0), 7).unwrap();
        assert_eq!(g.generate().len(), 7);
        let g = FibonacciEllipse::new(millimeter!(1.0), millimeter!(1.0), 19).unwrap();
        assert_eq!(g.generate().len(), 19);
    }
    #[test]
    fn generate_ellipse() {
        let g = FibonacciEllipse::new(millimeter!(1.0), millimeter!(1.0), 7).unwrap();
        assert_eq!(g.generate().len(), 7);
        let g = FibonacciEllipse::new(millimeter!(1.0), millimeter!(1.0), 19).unwrap();
        assert_eq!(g.generate().len(), 19);
    }
}
