//! Cylindrical surface
//!
//! This module implements a cylindrical surface with a given radius of curvature and a given position / alignment in 3D space.
use super::geo_surface::GeoSurface;
use crate::{
    error::{OpmResult, OpossumError},
    meter, radian,
    ray::Ray,
    utils::geom_transformation::Isometry,
};
use nalgebra::{Point3, Vector3, vector};
use num::Zero;
use roots::{Roots, find_roots_quadratic};
use uom::si::f64::Length;

#[derive(Debug, Clone)]
/// A cylindracal surface with its anchor point on the optical axis.
pub struct Cylinder {
    radius: Length,
    isometry: Isometry,
}
impl Cylinder {
    /// Create a new [`Cylinder`] located and oriented by the given [`Isometry`].
    ///
    /// **Note**: The anchor point is the center of the cylinder.
    ///
    /// # Errors
    ///
    /// This function will return an error if any components of the `pos` are not finite or if the radius is not normal.
    pub fn new(radius: Length, isometry: Isometry) -> OpmResult<Self> {
        if !radius.is_normal() {
            return Err(OpossumError::Other(
                "radius of curvature must be!=> 0.0 and finite".into(),
            ));
        }
        Ok(Self { radius, isometry })
    }
    /// Returns the center position of this [`Cylinder`]
    #[must_use]
    pub fn get_pos(&self) -> Point3<Length> {
        self.isometry.transform_point(&Point3::origin())
    }
}

impl GeoSurface for Cylinder {
    fn calc_intersect_and_normal_do(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        let dir = ray.direction();
        let pos = vector![
            ray.position().x.value,
            ray.position().y.value,
            ray.position().z.value
        ];
        let radius = self.radius.value;
        let is_back_propagating = dir.z.is_sign_negative();
        // cylinder formula (at origin) with the non-curved direction oriented along the y axis
        // x^2 + z^2 = r^2
        //
        // insert ray (p: position, d: direction):
        // (p_x+t*d_x)^2 + (p_z+t*d_z)^2 - r^2 = 0
        // This translates into the qudratic equation
        // at^2 + bt + c = 0 with
        // a = d_x^2 + d_z^2
        // b = 2 (d_x * p_x + d_z *p_z )
        // c = p_x^2 + p_z^2 - r^2

        let a = dir.x.mul_add(dir.x, dir.z * dir.z);
        let b = 2.0 * dir.x.mul_add(pos.x, dir.z * pos.z);
        let c = radius.mul_add(-radius, pos.x.mul_add(pos.x, pos.z * pos.z));
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
        // remove y component
        normal_vector.y = 0.0;
        normal_vector = normal_vector.normalize();
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
    fn set_isometry(&mut self, isometry: Isometry) {
        let anchor_isometry = Isometry::new(
            Point3::new(Length::zero(), Length::zero(), self.radius),
            radian!(0., 0., 0.),
        )
        .unwrap();
        self.isometry = isometry.append(&anchor_isometry);
    }
    fn isometry(&self) -> &Isometry {
        &self.isometry
    }

    fn name(&self) -> String {
        "cylindric".into()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{joule, millimeter, nanometer};
    use approx::assert_abs_diff_eq;

    #[test]
    fn new() {
        let iso = Isometry::new_along_z(millimeter!(1.0)).unwrap();
        assert!(Cylinder::new(millimeter!(f64::NAN), iso.clone()).is_err());
        assert!(Cylinder::new(millimeter!(f64::INFINITY), iso.clone()).is_err());
        assert!(Cylinder::new(millimeter!(f64::NEG_INFINITY), iso.clone()).is_err());

        let s = Cylinder::new(millimeter!(2.0), iso.clone()).unwrap();
        assert_eq!(s.radius, millimeter!(2.0));
        assert_eq!(s.get_pos(), millimeter!(0.0, 0.0, 1.0));

        let iso = Isometry::new_along_z(millimeter!(-1.0)).unwrap();

        let s = Cylinder::new(millimeter!(2.0), iso).unwrap();
        assert_eq!(s.radius, millimeter!(2.0));
        assert_eq!(s.get_pos(), millimeter!(0.0, 0.0, -1.0));
    }
    #[test]
    fn intersect_positive_on_axis() {
        let iso = Isometry::new_along_z(millimeter!(10.0)).unwrap();
        let s = Cylinder::new(millimeter!(1.0), iso).unwrap();
        let ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0)).unwrap();
        let (intersection_point, normal) = s.calc_intersect_and_normal(&ray).unwrap();
        assert_abs_diff_eq!(intersection_point.x.value, 0.0);
        assert_abs_diff_eq!(intersection_point.y.value, 0.0);
        assert_abs_diff_eq!(intersection_point.z.value, 0.009);
        assert_abs_diff_eq!(normal.x, 0.0);
        assert_abs_diff_eq!(normal.y, 0.0);
        assert_abs_diff_eq!(normal.z, -1.0);

        let ray = Ray::new_collimated(millimeter!(0.0, 1.0, 0.0), nanometer!(1053.0), joule!(1.0))
            .unwrap();
        let (intersection_point, normal) = s.calc_intersect_and_normal(&ray).unwrap();
        assert_abs_diff_eq!(intersection_point.x.value, 0.0);
        assert_abs_diff_eq!(intersection_point.y.value, 0.001);
        assert_abs_diff_eq!(intersection_point.z.value, 0.009);
        assert_abs_diff_eq!(normal.x, 0.0);
        assert_abs_diff_eq!(normal.y, 0.0);
        assert_abs_diff_eq!(normal.z, -1.0);
    }
    #[test]
    fn intersect_positive_on_axis_behind() {
        let ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0)).unwrap();
        let iso = Isometry::new_along_z(millimeter!(-10.0)).unwrap();
        let s = Cylinder::new(millimeter!(1.0), iso.clone()).unwrap();
        assert_eq!(s.calc_intersect_and_normal(&ray), None);
    }
    #[test]
    fn intersect_positive_collinear_no_intersect() {
        let iso = Isometry::new_along_z(millimeter!(10.0)).unwrap();
        let s = Cylinder::new(millimeter!(1.0), iso).unwrap();
        let ray = Ray::new_collimated(millimeter!(1.1, 0.0, 0.0), nanometer!(1053.0), joule!(1.0))
            .unwrap();
        assert_eq!(s.calc_intersect_and_normal(&ray), None);
    }
    #[test]
    fn intersect_positive_collinear_touch() {
        let wvl = nanometer!(1053.0);
        let ray = Ray::new_collimated(millimeter!(1.0, 0.0, 0.0), wvl, joule!(1.0)).unwrap();
        let iso = Isometry::new_along_z(millimeter!(10.0)).unwrap();
        let s = Cylinder::new(millimeter!(1.0), iso).unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((millimeter!(1.0, 0.0, 10.0), Vector3::x()))
        );
        let ray = Ray::new_collimated(millimeter!(-1.0, 0.0, 0.0), wvl, joule!(1.0)).unwrap();
        let (intersection_point, normal) = s.calc_intersect_and_normal(&ray).unwrap();
        assert_eq!(intersection_point.y, Length::zero());
        assert_abs_diff_eq!(intersection_point.x.value, -0.001);
        assert_abs_diff_eq!(
            intersection_point.z.value,
            0.01,
            epsilon = 1000.0 * f64::EPSILON
        );
        assert_abs_diff_eq!(normal.x, -1.0);
        assert_abs_diff_eq!(normal.y, 0.0);
        assert_abs_diff_eq!(normal.z, 0.0);
    }
}
