//! Circular and square, fibbonacci distribution
use std::f64::consts::PI;

use crate::error::{OpmResult, OpossumError};

use super::Distribution;
use nalgebra::{point, Point3};
use num::{ToPrimitive, Zero};
use uom::si::f64::Length;

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
    ///  - the given `side_length_x` or `side_length_y` is negative or not finite.
    pub fn new(side_length_x: Length, side_length_y: Length, nr_of_rays: usize) -> OpmResult<Self> {
        if side_length_x.is_sign_negative()
            || !side_length_x.is_finite()
            || side_length_y.is_sign_negative()
            || !side_length_y.is_finite()
        {
            return Err(OpossumError::Other(
                "side length must be positive and finite".into(),
            ));
        }
        Ok(Self {
            nr_of_rays,
            side_length_x,
            side_length_y,
        })
    }
}
impl Distribution for FibonacciRectangle {
    fn generate(&self) -> Vec<Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::with_capacity(self.nr_of_rays);
        let golden_ratio = (1. + f64::sqrt(5.)) / 2.;
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
    ///  - the given `side_length_x` or `side_length_y` is negative or not finite.
    pub fn new(radius_x: Length, radius_y: Length, nr_of_rays: usize) -> OpmResult<Self> {
        if radius_x.is_sign_negative()
            || !radius_x.is_finite()
            || radius_y.is_sign_negative()
            || !radius_y.is_finite()
        {
            return Err(OpossumError::Other(
                "radius must be positive and finite".into(),
            ));
        }
        Ok(Self {
            nr_of_rays,
            radius_x,
            radius_y,
        })
    }
}

impl Distribution for FibonacciEllipse {
    fn generate(&self) -> Vec<Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::with_capacity(self.nr_of_rays);
        let golden_ratio = (1. + f64::sqrt(5.)) / 2.;
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
