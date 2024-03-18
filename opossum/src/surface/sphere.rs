//! Spherical surface
//!
//! This module implements a spherical surface with a given radius of curvature and a given z position on the optical axis.
use super::Surface;
use crate::error::OpmResult;
use crate::error::OpossumError;
use crate::ray::Ray;
use nalgebra::Point3;
use nalgebra::Vector3;
use roots::find_roots_quadratic;
use roots::Roots;
use uom::si::f64::Length;
use uom::si::length::meter;

#[derive(Debug)]
/// A spherical surface with its origin on the optical axis.
pub struct Sphere {
    z: Length,
    radius: Length,
}
impl Sphere {
    /// Generate a new [`Sphere`] surface with a given z position on the optical axis and a given radius of curvature.
    ///
    /// # Errors
    ///
    /// This function will return an error if the radius of curvature is 0.0 or not finite.
    pub fn new(z: Length, radius_of_curvature: Length) -> OpmResult<Self> {
        if !radius_of_curvature.is_normal() {
            return Err(OpossumError::Other(
                "radius of curvature must be != 0.0 and finite".into(),
            ));
        }
        Ok(Self {
            z: z + radius_of_curvature,
            radius: radius_of_curvature,
        })
    }
}
impl Surface for Sphere {
    fn calc_intersect_and_normal(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        // sphere formula
        // x^2 + y^2 + (z-z_0)^2 = r^2
        //
        // insert ray (p: position, d: direction):
        // (p_x+t*d_x)^2 + (p_y+t*d_y)^2 + (p_z+t*d_z-z_0)^2 - r^2 = 0
        // This translates into the qudratic equation
        // at^2 + bt + c = 0 with
        // a = d_x^2 + d_y^2 + d_z^2
        // b = 2 (d_x * p_x + d_y * p_y + d_z *(p_z - z_0))
        // c = p_x^2 + p_y^2 + p_z^2 - 2*z_0*p_z + z_0^2 - r^2
        let factor = 1000.0;
        let dir = ray.direction();
        let pos = Vector3::new(
            ray.position().x.value * factor,
            ray.position().y.value * factor,
            ray.position().z.value * factor,
        );
        let z_0 = self.z.value * factor;
        let radius = self.radius.value * factor;
        let a = dir.norm_squared();
        let b = 2.0 * dir.z.mul_add(-z_0, dir.dot(&pos));
        let c = radius.mul_add(
            -radius,
            z_0.mul_add(z_0, (2.0 * z_0).mul_add(-pos.z, pos.norm_squared())),
        );
        // Solve t of qudaratic equation
        let roots = find_roots_quadratic(a, b, c);
        let intersection_point = match roots {
            // no intersection
            Roots::No(_) => return None,
            // "just touching" intersection
            Roots::One(t) => {
                if t[0] >= 0.0 {
                    (pos + t[0] * dir) / factor
                } else {
                    return None;
                }
            }
            // "regular" intersection
            Roots::Two(t) => {
                let real_t = if self.radius.is_sign_positive() {
                    // convex surface => use min t
                    f64::min(t[0], t[1])
                } else {
                    // concave surface => use max t
                    f64::max(t[0], t[1])
                };
                if real_t.is_sign_negative() {
                    // surface behind beam
                    return None;
                }
                (pos + real_t * dir) / factor
            }
            _ => unreachable!(),
        };
        let center_point = Vector3::new(0.0, 0.0, z_0 / factor);
        let mut normal_vector = (Vector3::from(intersection_point) - center_point).normalize();
        if self.radius.is_sign_negative() {
            normal_vector *= -1.0;
        }
        Some((
            Point3::new(
                Length::new::<meter>(intersection_point.x),
                Length::new::<meter>(intersection_point.y),
                Length::new::<meter>(intersection_point.z),
            ),
            normal_vector,
        ))
    }
}

#[cfg(test)]
mod test {
    use approx::assert_abs_diff_eq;
    use num::Zero;
    use uom::si::{
        energy::joule,
        f64::{Energy, Length},
        length::millimeter,
        length::nanometer,
    };

    use super::*;
    #[test]
    fn new() {
        let s = Sphere::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(2.0),
        )
        .unwrap();
        assert_eq!(s.z, Length::new::<millimeter>(3.0));
        assert_eq!(s.radius, Length::new::<millimeter>(2.0));
        assert!(Sphere::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(0.0)
        )
        .is_err());
        assert!(Sphere::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(f64::NAN)
        )
        .is_err());
        assert!(Sphere::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(f64::INFINITY)
        )
        .is_err());
        assert!(Sphere::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(f64::NEG_INFINITY)
        )
        .is_err());
        let s = Sphere::new(
            Length::new::<millimeter>(1.0),
            Length::new::<millimeter>(-2.0),
        )
        .unwrap();
        assert_eq!(s.z, Length::new::<millimeter>(-1.0));
        assert_eq!(s.radius, Length::new::<millimeter>(-2.0));
    }
    #[test]
    fn intersect_positive_on_axis() {
        let s = Sphere::new(
            Length::new::<millimeter>(10.0),
            Length::new::<millimeter>(1.0),
        )
        .unwrap();
        let ray = Ray::new(
            Point3::new(
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(0.0),
            ),
            Vector3::new(0.0, 0.0, 1.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let (intersection_point, normal) = s.calc_intersect_and_normal(&ray).unwrap();
        assert_abs_diff_eq!(intersection_point.x.value, 0.0);
        assert_abs_diff_eq!(intersection_point.y.value, 0.0);
        assert_abs_diff_eq!(intersection_point.z.value, 0.01);
        assert_abs_diff_eq!(normal.x, 0.0);
        assert_abs_diff_eq!(normal.y, 0.0);
        assert_abs_diff_eq!(normal.z, -1.0);
    }
    #[test]
    fn intersect_positive_on_axis_behind() {
        let ray = Ray::new(
            Point3::new(
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(0.0),
            ),
            Vector3::new(0.0, 0.0, 1.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let s = Sphere::new(
            Length::new::<millimeter>(-10.0),
            Length::new::<millimeter>(1.0),
        )
        .unwrap();
        assert_eq!(s.calc_intersect_and_normal(&ray), None);
        let s = Sphere::new(
            Length::new::<millimeter>(-10.0),
            Length::new::<millimeter>(-1.0),
        )
        .unwrap();
        assert_eq!(s.calc_intersect_and_normal(&ray), None);
    }
    #[test]
    fn intersect_positive_collinear_no_intersect() {
        let ray = Ray::new(
            Point3::new(
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(1.1),
                Length::new::<millimeter>(0.0),
            ),
            Vector3::new(0.0, 0.0, 1.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let s = Sphere::new(
            Length::new::<millimeter>(10.0),
            Length::new::<millimeter>(1.0),
        )
        .unwrap();
        assert_eq!(s.calc_intersect_and_normal(&ray), None);
        let ray = Ray::new(
            Point3::new(
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(-1.1),
                Length::new::<millimeter>(0.0),
            ),
            Vector3::new(0.0, 0.0, 1.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        assert_eq!(s.calc_intersect_and_normal(&ray), None);
    }
    #[test]
    fn intersect_positive_collinear_touch() {
        let wvl = Length::new::<nanometer>(1053.0);
        let ray = Ray::new(
            Point3::new(
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(0.0),
            ),
            Vector3::z(),
            wvl,
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let s = Sphere::new(
            Length::new::<millimeter>(10.0),
            Length::new::<millimeter>(1.0),
        )
        .unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((
                Point3::new(
                    Length::new::<millimeter>(0.0),
                    Length::new::<millimeter>(1.0),
                    Length::new::<millimeter>(11.0)
                ),
                Vector3::y()
            ))
        );
        let ray = Ray::new(
            Point3::new(
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(-1.0),
                Length::new::<millimeter>(-1.0),
            ),
            Vector3::z(),
            wvl,
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let (intersection_point, normal) = s.calc_intersect_and_normal(&ray).unwrap();
        assert_eq!(intersection_point.x, Length::zero());
        assert_abs_diff_eq!(intersection_point.y.value, -0.001);
        assert_abs_diff_eq!(
            intersection_point.z.value,
            0.011,
            epsilon = 1000.0 * f64::EPSILON
        );
        assert_abs_diff_eq!(normal.x, 0.0);
        assert_abs_diff_eq!(normal.y, -1.0);
        assert_abs_diff_eq!(normal.z, 0.0);
    }
    #[test]
    fn intersect_negative_on_axis() {
        let s = Sphere::new(
            Length::new::<millimeter>(10.0),
            Length::new::<millimeter>(-1.0),
        )
        .unwrap();
        let ray = Ray::new(
            Point3::new(
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(0.0),
            ),
            Vector3::new(0.0, 0.0, 1.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        let (intersection_point, normal) = s.calc_intersect_and_normal(&ray).unwrap();
        assert_abs_diff_eq!(intersection_point.x.value, 0.0);
        assert_abs_diff_eq!(intersection_point.y.value, 0.0);
        assert_abs_diff_eq!(intersection_point.z.value, 0.01);
        assert_abs_diff_eq!(normal.x, 0.0);
        assert_abs_diff_eq!(normal.y, 0.0);
        assert_abs_diff_eq!(normal.z, -1.0);
    }
}
