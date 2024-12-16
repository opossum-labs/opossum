//! Spherical surface
//!
//! This module implements a spherical surface with a given radius of curvature.
use super::geo_surface::GeoSurface;
use crate::{
    error::{OpmResult, OpossumError},
    meter, radian,
    ray::Ray,
    utils::geom_transformation::Isometry,
};
use nalgebra::{vector, Point3, Vector3};
use num::Zero;
use roots::{find_roots_quadratic, Roots};
use uom::si::f64::Length;

#[derive(Debug, Clone)]
/// A spherical surface with its anchor point directly on the surface.
pub struct Sphere {
    radius: Length,
    isometry: Isometry,
}
impl Sphere {
    /// Create a new [`Sphere`] located at a given position.
    ///
    /// **Note**: The anchor point is not the center of the sphere but a point on the sphere surface.
    ///
    /// # Errors
    ///
    /// This function will return an error if any components of the `pos` are not finite or if the radius is not normal.
    pub fn new_at_position(radius: Length, pos: Point3<Length>) -> OpmResult<Self> {
        if !radius.is_normal() {
            return Err(OpossumError::Other(
                "radius of curvature must be != 0.0 and finite".into(),
            ));
        }
        let isometry = Isometry::new(
            Point3::new(pos.x, pos.y, pos.z + radius),
            radian!(0., 0., 0.),
        )?;
        Ok(Self { radius, isometry })
    }
    /// Create a new [`Sphere`] located and oriented by the given [`Isometry`].
    ///
    /// **Note**: The anchor point is not the center of the sphere but a point on the sphere surface.
    ///
    /// # Errors
    ///
    /// This function will return an error if any components of the `pos` are not finite or if the radius is not normal.
    pub fn new(radius: Length, isometry: Isometry) -> OpmResult<Self> {
        if !radius.is_normal() {
            return Err(OpossumError::Other(
                "radius of curvature must be != 0.0 and finite".into(),
            ));
        }
        Ok(Self { radius, isometry })
    }
}
impl GeoSurface for Sphere {
    fn calc_intersect_and_normal_do(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        let dir = ray.direction();
        let pos = vector![
            ray.position().x.value,
            ray.position().y.value,
            ray.position().z.value
        ];
        let radius = self.radius.value;
        let is_back_propagating = dir.z.is_sign_negative();
        // sphere formula (at origin)
        // x^2 + y^2 + z^2 = r^2
        //
        // insert ray (p: position, d: direction):
        // (p_x+t*d_x)^2 + (p_y+t*d_y)^2 + (p_z+t*d_z)^2 - r^2 = 0
        // This translates into the qudratic equation
        // at^2 + bt + c = 0 with
        // a = d_x^2 + d_y^2 + d_z^2
        // b = 2 (d_x * p_x + d_y * p_y + d_z *p_z )
        // c = p_x^2 + p_y^2 + p_z^2 - r^2
        let a = dir.norm_squared();
        let b = 2.0 * dir.z.mul_add(0.0, dir.dot(&pos));
        let c = radius.mul_add(-radius, pos.norm_squared());
        // Solve t of qudaratic equation
        let roots = find_roots_quadratic(a, b, c);
        let intersection_point = match roots {
            // no intersection
            Roots::No(_) => return None,
            // "just touching" intersection
            Roots::One(t) => {
                if t[0] >= 0.0 {
                    pos + t[0] * dir
                } else {
                    return None;
                }
            }
            // "regular" intersection
            Roots::Two(t) => {
                let real_t = if self.radius.is_sign_positive() {
                    // convex surface => use min t
                    if is_back_propagating {
                        f64::max(t[0], t[1])
                    } else {
                        f64::min(t[0], t[1])
                    }
                } else {
                    // concave surface => use max t
                    if is_back_propagating {
                        f64::min(t[0], t[1])
                    } else {
                        f64::max(t[0], t[1])
                    }
                };
                if real_t.is_sign_negative() {
                    // surface behind beam
                    return None;
                }
                pos + real_t * dir
            }
            _ => unreachable!(),
        };
        let mut normal_vector = intersection_point.normalize();
        if self.radius.is_sign_negative() {
            if is_back_propagating {
            } else {
                normal_vector *= -1.0;
            }
        }
        if self.radius.is_sign_positive() && is_back_propagating {
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
    fn set_isometry(&mut self, isometry: &Isometry) {
        let anchor_isometry = Isometry::new(
            Point3::new(Length::zero(), Length::zero(), self.radius),
            radian!(0., 0., 0.),
        )
        .unwrap();
        self.isometry = isometry.clone().append(&anchor_isometry);
    }
    fn isometry(&self) -> &Isometry {
        &self.isometry
    }
}

// impl Color for Sphere {
//     fn get_color(&self, _p: &Point3<f64>) -> Vector3<f64> {
//         Vector3::new(0.5, 0.3, 0.5)
//     }
// }
// impl SDF for Sphere {
//     fn sdf_eval_point(&self, p: &Point3<f64>) -> f64 {
//         let p_out = self.isometry.inverse_transform_point_f64(p);
//         (p_out
//             .x
//             .mul_add(p_out.x, p_out.y.mul_add(p_out.y, p_out.z * p_out.z)))
//         .sqrt()
//             - self.radius.value
//     }
// }
#[cfg(test)]
mod test {
    use super::*;
    use crate::{joule, millimeter, nanometer};
    use approx::assert_abs_diff_eq;

    #[test]
    fn new() {
        let iso = Isometry::new_along_z(millimeter!(1.0)).unwrap();
        assert!(Sphere::new(millimeter!(f64::NAN), iso.clone()).is_err());
        assert!(Sphere::new(millimeter!(f64::INFINITY), iso.clone()).is_err());
        assert!(Sphere::new(millimeter!(f64::NEG_INFINITY), iso.clone()).is_err());

        let s = Sphere::new(millimeter!(2.0), iso.clone()).unwrap();
        assert_eq!(s.radius, millimeter!(2.0));
    }
    #[test]
    fn new_at_position() {
        assert!(
            Sphere::new_at_position(millimeter!(f64::NAN), millimeter!(0.0, 0.0, 0.0)).is_err()
        );
        assert!(
            Sphere::new_at_position(millimeter!(f64::INFINITY), millimeter!(0.0, 0.0, 0.0))
                .is_err()
        );
        assert!(Sphere::new_at_position(
            millimeter!(f64::NEG_INFINITY),
            millimeter!(0.0, 0.0, 0.0)
        )
        .is_err());
        assert!(
            Sphere::new_at_position(millimeter!(1.0), millimeter!(f64::NAN, 0.0, 0.0)).is_err()
        );
        assert!(
            Sphere::new_at_position(millimeter!(1.0), millimeter!(f64::INFINITY, 0.0, 0.0))
                .is_err()
        );
        assert!(Sphere::new_at_position(
            millimeter!(1.0),
            millimeter!(f64::NEG_INFINITY, 0.0, 0.0)
        )
        .is_err());

        let s = Sphere::new_at_position(millimeter!(2.0), millimeter!(1.0, 2.0, 3.0)).unwrap();
        assert_eq!(s.radius, millimeter!(2.0));
    }
    #[test]
    fn intersect_positive_on_axis_forward() {
        let sphere_position = millimeter!(0.0, 0.0, 0.0);
        let s = Sphere::new_at_position(millimeter!(10.0), sphere_position).unwrap();

        // start "within" the sphere (not really)...
        let ray = Ray::new_collimated(millimeter!(0.0, 0.0, -5.0), nanometer!(1053.0), joule!(1.0))
            .unwrap();
        let (intersection_point, normal) = s.calc_intersect_and_normal(&ray).unwrap();
        assert_eq!(intersection_point, sphere_position);
        assert_eq!(normal, vector![0.0, 0.0, -1.0]);

        // start "outside" the sphere
        let ray = Ray::new_collimated(
            millimeter!(0.0, 0.0, -15.0),
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        let (intersection_point, _) = s.calc_intersect_and_normal(&ray).unwrap();
        assert_abs_diff_eq!(intersection_point.x.value, sphere_position.x.value);
        assert_abs_diff_eq!(intersection_point.y.value, sphere_position.y.value);
        assert_abs_diff_eq!(intersection_point.z.value, sphere_position.z.value);

        // non-intersecting
        let ray = Ray::new_collimated(millimeter!(0.0, 0.0, 5.0), nanometer!(1053.0), joule!(1.0))
            .unwrap();
        assert!(s.calc_intersect_and_normal(&ray).is_none());
        let ray = Ray::new_collimated(millimeter!(0.0, 0.0, 15.0), nanometer!(1053.0), joule!(1.0))
            .unwrap();
        assert!(s.calc_intersect_and_normal(&ray).is_none());
    }
    #[test]
    fn intersect_negative_on_axis_forward() {
        let sphere_position = millimeter!(0.0, 0.0, 0.0);
        let s = Sphere::new_at_position(millimeter!(-10.0), sphere_position).unwrap();

        // start "within" the sphere (not really)...
        let ray = Ray::new_collimated(millimeter!(0.0, 0.0, -5.0), nanometer!(1053.0), joule!(1.0))
            .unwrap();
        let (intersection_point, normal) = s.calc_intersect_and_normal(&ray).unwrap();
        assert_eq!(intersection_point, sphere_position);
        assert_eq!(normal, vector![0.0, 0.0, -1.0]);

        // start "outside" the sphere
        let ray = Ray::new_collimated(
            millimeter!(0.0, 0.0, -15.0),
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        let (intersection_point, _) = s.calc_intersect_and_normal(&ray).unwrap();
        assert_abs_diff_eq!(intersection_point.x.value, sphere_position.x.value);
        assert_abs_diff_eq!(intersection_point.y.value, sphere_position.y.value);
        assert_abs_diff_eq!(intersection_point.z.value, sphere_position.z.value);

        // non-intersecting
        let ray = Ray::new_collimated(millimeter!(0.0, 0.0, 5.0), nanometer!(1053.0), joule!(1.0))
            .unwrap();
        assert!(s.calc_intersect_and_normal(&ray).is_none());
        let ray = Ray::new_collimated(millimeter!(0.0, 0.0, 15.0), nanometer!(1053.0), joule!(1.0))
            .unwrap();
        assert!(s.calc_intersect_and_normal(&ray).is_none());
    }
    #[test]
    fn intersect_positive_on_axis_backward() {
        let sphere_position = millimeter!(0.0, 0.0, 0.0);
        let s = Sphere::new_at_position(millimeter!(10.0), sphere_position).unwrap();

        // start "within" the sphere (not really)...
        let ray = Ray::new(
            millimeter!(0.0, 0.0, 5.0),
            vector![0.0, 0.0, -1.0],
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        let (intersection_point, normal) = s.calc_intersect_and_normal(&ray).unwrap();
        assert_eq!(intersection_point, sphere_position);
        assert_eq!(normal, vector![0.0, 0.0, 1.0]);

        // start "outside" the sphere
        let ray = Ray::new(
            millimeter!(0.0, 0.0, 15.0),
            vector![0.0, 0.0, -1.0],
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        let (intersection_point, _) = s.calc_intersect_and_normal(&ray).unwrap();
        assert_abs_diff_eq!(intersection_point.x.value, sphere_position.x.value);
        assert_abs_diff_eq!(intersection_point.y.value, sphere_position.y.value);
        assert_abs_diff_eq!(intersection_point.z.value, sphere_position.z.value);

        // non-intersecting
        let ray = Ray::new(
            millimeter!(0.0, 0.0, -5.0),
            vector![0.0, 0.0, -1.0],
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        assert!(s.calc_intersect_and_normal(&ray).is_none());
        let ray = Ray::new(
            millimeter!(0.0, 0.0, -15.0),
            vector![0.0, 0.0, -1.0],
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        assert!(s.calc_intersect_and_normal(&ray).is_none());
    }
    #[test]
    fn intersect_negative_on_axis_backward() {
        let sphere_position = millimeter!(0.0, 0.0, 0.0);
        let s = Sphere::new_at_position(millimeter!(-10.0), sphere_position).unwrap();

        // start "within" the sphere (not really)...
        let ray = Ray::new(
            millimeter!(0.0, 0.0, 5.0),
            vector![0.0, 0.0, -1.0],
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        let (intersection_point, normal) = s.calc_intersect_and_normal(&ray).unwrap();
        assert_eq!(intersection_point, sphere_position);
        assert_eq!(normal, vector![0.0, 0.0, 1.0]);

        // start "outside" the sphere
        let ray = Ray::new(
            millimeter!(0.0, 0.0, 15.0),
            vector![0.0, 0.0, -1.0],
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        let (intersection_point, _) = s.calc_intersect_and_normal(&ray).unwrap();
        assert_abs_diff_eq!(intersection_point.x.value, sphere_position.x.value);
        assert_abs_diff_eq!(intersection_point.y.value, sphere_position.y.value);
        assert_abs_diff_eq!(intersection_point.z.value, sphere_position.z.value);

        // non-intersecting
        let ray = Ray::new(
            millimeter!(0.0, 0.0, -5.0),
            vector![0.0, 0.0, -1.0],
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        assert!(s.calc_intersect_and_normal(&ray).is_none());
        let ray = Ray::new(
            millimeter!(0.0, 0.0, -15.0),
            vector![0.0, 0.0, -1.0],
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        assert!(s.calc_intersect_and_normal(&ray).is_none());
    }
    #[test]
    fn intersect_positive_collinear_no_intersect() {
        let ray = Ray::new_collimated(millimeter!(0.0, 1.1, 0.0), nanometer!(1053.0), joule!(1.0))
            .unwrap();
        let s = Sphere::new_at_position(millimeter!(1.0), millimeter!(0.0, 0.0, 10.0)).unwrap();
        assert!(s.calc_intersect_and_normal(&ray).is_none());
        let ray = Ray::new_collimated(millimeter!(0.0, -1.1, 0.0), nanometer!(1053.0), joule!(1.0))
            .unwrap();
        assert!(s.calc_intersect_and_normal(&ray).is_none());
    }
    #[test]
    fn intersect_positive_collinear_touch() {
        let wvl = nanometer!(1053.0);
        let ray = Ray::new_collimated(millimeter!(0.0, 1.0, 0.0), wvl, joule!(1.0)).unwrap();
        let s = Sphere::new_at_position(millimeter!(1.0), millimeter!(0.0, 0.0, 10.0)).unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((millimeter!(0.0, 1.0, 11.0), Vector3::y()))
        );
        let ray = Ray::new_collimated(millimeter!(0.0, -1.0, -1.0), wvl, joule!(1.0)).unwrap();
        let (intersection_point, normal) = s.calc_intersect_and_normal(&ray).unwrap();
        assert_eq!(intersection_point.x, Length::zero());
        assert_abs_diff_eq!(intersection_point.y.value, -0.001);
        assert_abs_diff_eq!(
            intersection_point.z.value,
            0.011,
            epsilon = 1000.0 * f64::EPSILON
        );
        assert_abs_diff_eq!(normal, vector![0.0, -1.0, 0.0]);
    }
}
