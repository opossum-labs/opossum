//! Circular, hexapolar distribution
use std::f64::consts::PI;

use crate::error::{OpmResult, OpossumError};

use super::PositionDistribution;
use nalgebra::{Point3, Vector3};
use num::{ToPrimitive, Zero};
use uom::si::f64::Length;

/// Circular, hexapolar distribution
#[derive(Clone)]
pub struct HexagonalTiling {
    nr_of_hex_along_radius: u8,
    radius: Length,
}
impl HexagonalTiling {
    /// Create a new [`HexagonalTiling`] distribution generator.
    ///
    /// If the given radius is zero and / or `nr_of_rings` is zero only the central point at (0,0) is generated.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///  - the given `radius` is negative or not finite.
    pub fn new(radius: Length, nr_of_hex_along_radius: u8) -> OpmResult<Self> {
        if radius.is_sign_negative() || !radius.is_finite() {
            return Err(OpossumError::Other(
                "radius must be positive and finite".into(),
            ));
        }
        Ok(Self {
            nr_of_hex_along_radius,
            radius,
        })
    }
}

impl PositionDistribution for HexagonalTiling {
    fn generate(&self) -> Vec<Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::new();
        // Add center point
        points.push(Point3::origin());

        let radius_step = self.radius / self.nr_of_hex_along_radius.to_f64().unwrap();
        let mut i = 1;
        let border_radius = self.radius * 5.0f64.mul_add(f64::EPSILON, 1.);
        loop {
            let mut all_outside_radius = true;
            let mut hex = Point3::<Length>::origin();
            hex.x = radius_step * i.to_f64().unwrap();
            for j in 0_u8..6 {
                let angle = PI / 3. * (2. + j.to_f64().unwrap());
                let shift_vec = Vector3::new(
                    f64::cos(angle) * radius_step,
                    f64::sin(angle) * radius_step,
                    Length::zero(),
                );
                for _k in 0_u8..i {
                    if (hex.x * hex.x + hex.y * hex.y).sqrt() <= border_radius {
                        points.push(hex);
                        all_outside_radius = false;
                    }
                    hex += shift_vec;
                }
            }
            if all_outside_radius {
                break;
            }
            i += 1;
        }
        points
    }
    // fn generate(&self) -> Vec<Point3<Length>> {
    //     let mut points: Vec<Point3<Length>> = Vec::new();
    //     // Add center point
    //     points.push(Point3::origin());

    //     let radius_step = self.radius/self.nr_of_hex_along_radius.to_f64().unwrap();
    //     for i in 1_u8..self.nr_of_hex_along_radius+1{
    //         // let mut last_point = points.last().unwrap().clone();
    //         // last_point.x += radius_step;
    //         let mut hex = Point3::origin();
    //         hex.x += radius_step*i.to_f64().unwrap();
    //         for j in 0_u8..6{
    //             for k in 0_u8..i{
    //                 points.push(hex);
    //                 let angle = PI/3.*(2.+j.to_f64().unwrap());
    //                 hex = hex + Vector3::new(f64::cos(angle)*radius_step, f64::sin(angle)*radius_step,Length::zero());
    //             }
    //         }
    //     }
    //     points
    // }
}
