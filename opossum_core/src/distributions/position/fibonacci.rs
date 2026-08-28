#![warn(missing_docs)]
//! Circular and square, fibbonacci distribution
use std::f64::consts::PI;

use crate::{
    error::OpmResult,
    generic_validators::{AllFinite, AllNotZero, AllPositive, NotAllZero},
    millimeter, validated, validated_type,
};

use super::PositionDistribution;
use nalgebra::{Point2, Point3, point};
use num::{ToPrimitive, Zero};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

/// Rectangular Fibonacci distribution
///
/// For further details see [here](https://en.wikipedia.org/wiki/Fibonacci_sequence)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Copy, EnsureValidated)]
pub struct FibonacciRectangle {
    nr_of_points: validated_type!(usize, AllNotZero),
    side_length: validated_type!(Point2<Length>, NotAllZero && AllFinite && AllPositive),
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
    ///  - the given `nr_of_points` is zero.
    pub fn new(
        side_length_x: Length,
        side_length_y: Length,
        nr_of_points: usize,
    ) -> OpmResult<Self> {
        let mut fibonacci_rect = Self::default();
        fibonacci_rect.set_nr_of_points(nr_of_points)?;
        fibonacci_rect.set_side_length_x(side_length_x)?;
        fibonacci_rect.set_side_length_y(side_length_y)?;
        Ok(fibonacci_rect)
    }

    /// Returns the number of points (rays) in the Fibonacci rectangle distribution.
    ///
    /// # Returns
    ///
    /// The number of points as a `usize`.
    #[must_use]
    pub const fn nr_of_points(&self) -> usize {
        *self.nr_of_points.get()
    }

    /// Returns the side length along the X axis of the rectangle.
    ///
    /// # Returns
    ///
    /// The length of the side in the X direction as a `Length`.
    ///
    /// # Errors
    /// Returns an error if validation fails
    #[must_use]
    pub fn side_length_x(&self) -> Length {
        self.side_length.get().x
    }

    /// Returns the side length along the Y axis of the rectangle.
    ///
    /// # Returns
    ///
    /// The length of the side in the Y direction as a `Length`.
    ///
    /// # Errors
    /// Returns an error if validation fails
    #[must_use]
    pub fn side_length_y(&self) -> Length {
        self.side_length.get().y
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
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_nr_of_points(&mut self, nr_of_points: usize) -> OpmResult<()> {
        self.nr_of_points.set(nr_of_points)?;
        Ok(())
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
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_side_length_x(&mut self, side_length_x: Length) -> OpmResult<()> {
        self.side_length
            .set(Point2::new(side_length_x, self.side_length_y()))?;
        Ok(())
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
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_side_length_y(&mut self, side_length_y: Length) -> OpmResult<()> {
        self.side_length
            .set(Point2::new(self.side_length_x(), side_length_y))?;
        Ok(())
    }
}

impl Default for FibonacciRectangle {
    fn default() -> Self {
        Self {
            nr_of_points: validated!(1000_usize, AllNotZero).unwrap(),
            side_length: validated!(millimeter!(5., 5.), NotAllZero && AllFinite && AllPositive)
                .unwrap(),
        }
    }
}

impl PositionDistribution for FibonacciRectangle {
    fn generate(&self) -> Vec<Point3<Length>> {
        let nr_of_rays = *self.nr_of_points.get();
        let nr_of_rays_f64 = nr_of_rays.to_f64().unwrap();
        let mut points: Vec<Point3<Length>> = Vec::with_capacity(nr_of_rays);
        let golden_ratio = f64::midpoint(1., f64::sqrt(5.));
        for i in 0_usize..nr_of_rays {
            let i_f64 = i.to_f64().unwrap();
            points.push(point![
                self.side_length_x() * ((i_f64 / golden_ratio).fract() - 0.5),
                self.side_length_y() * ((i_f64 / nr_of_rays_f64) - 0.5),
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
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Copy, EnsureValidated)]
pub struct FibonacciEllipse {
    nr_of_points: validated_type!(usize, AllNotZero),
    radius: validated_type!(Point2<Length>, NotAllZero && AllFinite && AllPositive),
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
        let mut fibonacci_ell = Self::default();
        fibonacci_ell.set_nr_of_points(nr_of_rays)?;
        fibonacci_ell.set_radius_x(radius_x)?;
        fibonacci_ell.set_radius_y(radius_y)?;
        Ok(fibonacci_ell)
    }
    /// Returns the number of points (rays) in the Fibonacci ellipse distribution.
    ///
    /// # Returns
    ///
    /// The number of points as a `usize`.
    #[must_use]
    pub const fn nr_of_points(&self) -> usize {
        *self.nr_of_points.get()
    }

    /// Returns the radius along the X axis of the ellipse.
    ///
    /// # Returns
    ///
    /// The radius in the X direction as a `Length`.
    #[must_use]
    pub fn radius_x(&self) -> Length {
        self.radius.get().x
    }

    /// Returns the radius along the Y axis of the ellipse.
    ///
    /// # Returns
    ///
    /// The radius in the Y direction as a `Length`.
    #[must_use]
    pub fn radius_y(&self) -> Length {
        self.radius.get().y
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
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_nr_of_points(&mut self, nr_of_points: usize) -> OpmResult<()> {
        self.nr_of_points.set(nr_of_points)?;
        Ok(())
    }

    /// Sets the radius along the X axis of the ellipse.
    ///
    /// # Parameters
    ///
    /// * `radius_x` - The new radius in the X direction.
    ///
    /// # Side Effects
    ///
    /// Updates the current `radius_x`.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_radius_x(&mut self, radius_x: Length) -> OpmResult<()> {
        self.radius.set(Point2::new(radius_x, self.radius_y()))?;
        Ok(())
    }

    /// Sets the radius along the Y axis of the ellipse.
    ///
    /// # Parameters
    ///
    /// * `radius_y` - The new radius in the Y direction.
    ///
    /// # Side Effects
    ///
    /// Updates the current `radius_y`.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_radius_y(&mut self, radius_y: Length) -> OpmResult<()> {
        self.radius.set(Point2::new(self.radius_x(), radius_y))?;
        Ok(())
    }
}

impl Default for FibonacciEllipse {
    fn default() -> Self {
        Self {
            nr_of_points: validated!(1000_usize, AllNotZero).unwrap(),
            radius: validated!(millimeter!(5., 5.), NotAllZero && AllFinite && AllPositive)
                .unwrap(),
        }
    }
}

impl PositionDistribution for FibonacciEllipse {
    fn generate(&self) -> Vec<Point3<Length>> {
        let nr_of_points = *self.nr_of_points.get();
        let nr_of_points_f64 = nr_of_points.to_f64().unwrap();
        let mut points: Vec<Point3<Length>> = Vec::with_capacity(nr_of_points);
        let golden_ratio = f64::midpoint(1., f64::sqrt(5.));
        for i in 0_usize..nr_of_points {
            let sin_cos = f64::sin_cos(2. * PI * (i.to_f64().unwrap() / golden_ratio).fract());
            let sqrt_r = f64::sqrt(i.to_f64().unwrap() / nr_of_points_f64);
            points.push(point![
                self.radius_x() * sin_cos.0 * sqrt_r,
                self.radius_y() * sin_cos.1 * sqrt_r,
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
    fn generate_rect() -> OpmResult<()> {
        let g = FibonacciEllipse::new(millimeter!(1.0), millimeter!(1.0), 7)?;
        assert_eq!(g.generate().len(), 7);
        let g = FibonacciEllipse::new(millimeter!(1.0), millimeter!(1.0), 19)?;
        assert_eq!(g.generate().len(), 19);
        Ok(())
    }
    #[test]
    fn generate_ellipse() -> OpmResult<()> {
        let g = FibonacciEllipse::new(millimeter!(1.0), millimeter!(1.0), 7)?;
        assert_eq!(g.generate().len(), 7);
        let g = FibonacciEllipse::new(millimeter!(1.0), millimeter!(1.0), 19)?;
        assert_eq!(g.generate().len(), 19);
        Ok(())
    }
}
