//! Spherical surface
//!
//! This module implements a spherical surface with a given radius of curvature and a given z position on the optical axis.
use super::Surface;
use crate::millimeter;
use crate::ray::Ray;
use crate::render::{Color, Render, Renderable, SDF};
use crate::utils::geom_transformation::Isometry;
use crate::{
    error::{OpmResult, OpossumError},
    meter,
};
use nalgebra::{Point3, Vector4};
use nalgebra::Vector3;
use ncollide2d::math::Vector;
use num::{Float, Zero};
use roots::find_roots_quadratic;
use roots::Roots;
use uom::si::f64::Length;

#[derive(Debug)]
/// A spherical surface with its origin on the optical axis.
pub struct Sphere {
    z: Length,
    radius: Length,
    pos: Point3<Length>,
    isometry: Isometry,
}
impl Sphere {
    /// Generate a new [`Sphere`] surface with a given z position on the optical axis and a given radius of curvature.
    ///
    /// # Errors
    ///
    /// This function will return an error if the radius of curvature is 0.0 or not finite.
    pub fn new_along_z(z: Length, radius_of_curvature: Length) -> OpmResult<Self> {
        if !radius_of_curvature.is_normal() {
            return Err(OpossumError::Other(
                "radius of curvature must be != 0.0 and finite".into(),
            ));
        }
        let isometry = Isometry::new(
            Point3::new(Length::zero(), Length::zero(), z),
            Vector3::new(0.,0.,0.),
        );

        Ok(Self {
            z: z + radius_of_curvature,
            radius: radius_of_curvature,
            pos: Point3::new(Length::zero(), Length::zero(), z),
            isometry,
        })
    }

    /// Create a new [`Sphere`] located at a given position.
    ///
    /// # Errors
    ///
    /// This function will return an error if any components of the `pos` are not finite or if the radius is not normal.
    pub fn new(radius: Length, pos: Point3<Length>) -> OpmResult<Self> {
        if !radius.is_normal() {
            return Err(OpossumError::Other(
                "radius of curvature must be != 0.0 and finite".into(),
            ));
        }
        if pos.iter().any(|x| !x.is_finite()) {
            return Err(OpossumError::Other("center point coordinates must be finite".into()));
        }
        let isometry = Isometry::new(pos, Vector3::new(0.,0.,0.));
        Ok(Self {  
            z: pos.z,
            radius,
            pos,
            isometry})
    }
}
impl Render<'_> for Sphere{}
impl Renderable<'_> for Sphere{}

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
            meter!(
                intersection_point.x,
                intersection_point.y,
                intersection_point.z
            ),
            normal_vector,
        ))
    }
}

impl Color for Sphere{
    fn get_color(&self, _p: &Point3<f64>) -> Vector3<f64> {
        Vector3::new(0.5,0.3,0.5)
    }
}
impl SDF for Sphere 
{
    fn sdf_eval_point(&self, p: &Point3<f64>, p_out: &mut Point3<f64>) -> f64 {
        self.isometry.inverse_transform_point_mut_f64(&p, p_out);
        // (p.x * p.x + p.y * p.y + p.z * p.z).sqrt() - self.radius.value
        (p_out.x.mul_add(p_out.x, p_out.y.mul_add(p_out.y, p_out.z*p_out.z)) ).sqrt() - self.radius.value
        // Vector4::<f64>::from_slice(&[self.get_color(&p).as_slice(),  &[(p.x * p.x + p.y * p.y + p.z * p.z).sqrt() - self.radius.value]].concat())

        }
}

#[cfg(test)]
mod test {
    use crate::{joule, millimeter, nanometer};
    use approx::assert_abs_diff_eq;
    use num::Zero;
    use uom::si::f64::Length;

    use super::*;
    #[test]
    fn new() {
        let s = Sphere::new_along_z(millimeter!(1.0), millimeter!(2.0)).unwrap();
        assert_eq!(s.z, millimeter!(3.0));
        assert_eq!(s.radius, millimeter!(2.0));
        assert!(Sphere::new_along_z(millimeter!(1.0), millimeter!(0.0)).is_err());
        assert!(Sphere::new_along_z(millimeter!(1.0), millimeter!(f64::NAN)).is_err());
        assert!(Sphere::new_along_z(millimeter!(1.0), millimeter!(f64::INFINITY)).is_err());
        assert!(Sphere::new_along_z(millimeter!(1.0), millimeter!(f64::NEG_INFINITY)).is_err());
        let s = Sphere::new_along_z(millimeter!(1.0), millimeter!(-2.0)).unwrap();
        assert_eq!(s.z, millimeter!(-1.0));
        assert_eq!(s.radius, millimeter!(-2.0));
    }
    #[test]
    fn intersect_positive_on_axis() {
        let s = Sphere::new_along_z(millimeter!(10.0), millimeter!(1.0)).unwrap();
        let ray = Ray::new(
            millimeter!(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            nanometer!(1053.0),
            joule!(1.0),
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
            millimeter!(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        let s = Sphere::new_along_z(millimeter!(-10.0), millimeter!(1.0)).unwrap();
        assert_eq!(s.calc_intersect_and_normal(&ray), None);
        let s = Sphere::new_along_z(millimeter!(-10.0), millimeter!(-1.0)).unwrap();
        assert_eq!(s.calc_intersect_and_normal(&ray), None);
    }
    #[test]
    fn intersect_positive_collinear_no_intersect() {
        let ray = Ray::new(
            millimeter!(0.0, 1.1, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        let s = Sphere::new_along_z(millimeter!(10.0), millimeter!(1.0)).unwrap();
        assert_eq!(s.calc_intersect_and_normal(&ray), None);
        let ray = Ray::new(
            millimeter!(0.0, -1.1, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        assert_eq!(s.calc_intersect_and_normal(&ray), None);
    }
    #[test]
    fn intersect_positive_collinear_touch() {
        let wvl = nanometer!(1053.0);
        let ray = Ray::new(millimeter!(0.0, 1.0, 0.0), Vector3::z(), wvl, joule!(1.0)).unwrap();
        let s = Sphere::new_along_z(millimeter!(10.0), millimeter!(1.0)).unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((millimeter!(0.0, 1.0, 11.0), Vector3::y()))
        );
        let ray = Ray::new(millimeter!(0.0, -1.0, -1.0), Vector3::z(), wvl, joule!(1.0)).unwrap();
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
        let s = Sphere::new_along_z(millimeter!(10.0), millimeter!(-1.0)).unwrap();
        let ray = Ray::new(
            millimeter!(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            nanometer!(1053.0),
            joule!(1.0),
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
