//! Cylindrical surface
//!
//! This module implements a cylindrical surface with a given radius of curvature and a given position / alignment in 3D space.
use super::geo_surface::{GeoSurface, curved_local_z, is_behind_curvature};
use crate::{
    error::{OpmResult, OpossumError},
    light::Ray,
    meter, radian,
    utils::geom_transformation::Isometry,
};
use nalgebra::{Point2, Point3, Vector3};
use num_traits::Zero;
use roots::{Roots, find_roots_quadratic};
use uom::si::f64::Length;

#[derive(Debug, Clone)]
/// A cylindrical surface with its anchor point on the optical axis.
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
                "radius of curvature must be != 0.0 and finite".into(),
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
    #[allow(clippy::suboptimal_flops)] // don't use mul_add here for a,b,c because the current implementation is faster!
    fn calc_intersect_and_normal_do(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        let dir = ray.direction();
        let pos_vec = ray.position().coords.map(|v| v.value);
        let radius = self.radius.value;

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

        let a = dir.x * dir.x + dir.z * dir.z;
        let b = 2.0 * (pos_vec.x * dir.x + pos_vec.z * dir.z);
        let c = pos_vec.x * pos_vec.x + pos_vec.z * pos_vec.z - radius * radius;

        // Robustness check for rays parallel to the axis.
        if a.abs() < 1e-9 {
            return None;
        }
        let roots = find_roots_quadratic(a, b, c);
        let is_back_propagating = dir.z.is_sign_negative();
        let real_t = match roots {
            Roots::No(_) => return None,
            Roots::One(t) => {
                if t[0] >= 0.0 {
                    t[0]
                } else {
                    return None;
                }
            }
            Roots::Two(t) => {
                if self.radius.is_sign_positive() {
                    // Convex surface
                    if is_back_propagating {
                        f64::max(t[0], t[1])
                    } else {
                        f64::min(t[0], t[1])
                    }
                } else {
                    // Concave surface
                    if is_back_propagating {
                        f64::min(t[0], t[1])
                    } else {
                        f64::max(t[0], t[1])
                    }
                }
            }
            _ => unreachable!(),
        };
        if real_t.is_sign_negative() {
            return None;
        }
        let intersection_point = pos_vec + real_t * dir;
        let mut normal = Vector3::new(intersection_point.x, 0.0, intersection_point.z).normalize();
        // The normal always "faces" the incoming ray
        if self.radius.is_sign_positive() {
            // Convex
            if is_back_propagating {
                normal.neg_mut();
            }
        } else {
            // Concave
            if !is_back_propagating {
                normal.neg_mut();
            }
        }
        Some((
            meter!(
                intersection_point.x,
                intersection_point.y,
                intersection_point.z
            ),
            normal,
        ))
    }

    fn local_z_at(&self, transversal_position: &Point2<Length>) -> Option<Length> {
        // The cylinder axis runs along y, so the surface is straight in that direction and reaches
        // arbitrarily far along it — only the x offset bends it.
        curved_local_z(transversal_position.x.value.abs(), self.radius.value).map(|z| meter!(z))
    }
    fn is_behind_do(&self, point: &Point3<Length>) -> bool {
        // The local origin lies on the cylinder axis which runs along y, so the surface is the
        // circle of |radius| in the xz plane.
        is_behind_curvature(point.x.value.hypot(point.z.value), self.radius.value)
    }
    fn set_isometry(&mut self, isometry: Isometry) {
        let anchor_isometry = Isometry::new(
            Point3::new(Length::zero(), Length::zero(), self.radius),
            radian!(0., 0., 0.),
        )
        .expect("Could not set anchor isometry");
        self.isometry = isometry.append(&anchor_isometry);
    }
    fn isometry(&self) -> &Isometry {
        &self.isometry
    }
    fn name(&self) -> &'static str {
        "cylindric"
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{joule, millimeter, nanometer};
    use approx::assert_abs_diff_eq;
    use uom::si::length::millimeter;

    #[test]
    fn new() -> OpmResult<()> {
        let iso = Isometry::new_along_z(millimeter!(1.0))?;
        assert!(Cylinder::new(millimeter!(f64::NAN), iso.clone()).is_err());
        assert!(Cylinder::new(millimeter!(f64::INFINITY), iso.clone()).is_err());
        assert!(Cylinder::new(millimeter!(f64::NEG_INFINITY), iso.clone()).is_err());

        let s = Cylinder::new(millimeter!(2.0), iso.clone())?;
        assert_eq!(s.radius, millimeter!(2.0));
        assert_eq!(s.get_pos(), millimeter!(0.0, 0.0, 1.0));

        let iso = Isometry::new_along_z(millimeter!(-1.0))?;

        let s = Cylinder::new(millimeter!(2.0), iso)?;
        assert_eq!(s.radius, millimeter!(2.0));
        assert_eq!(s.get_pos(), millimeter!(0.0, 0.0, -1.0));
        Ok(())
    }
    #[test]
    fn is_behind() -> OpmResult<()> {
        // cylinder axis at z = 10 mm, so the vertex of the convex surface lies at z = 9 mm
        let iso = Isometry::new_along_z(millimeter!(10.0))?;
        let s = Cylinder::new(millimeter!(1.0), iso)?;
        assert!(s.is_behind(&millimeter!(0.0, 0.0, 9.5)));
        // a point exactly on the surface counts as behind it
        assert!(s.is_behind(&millimeter!(0.0, 0.0, 9.0)));
        assert!(!s.is_behind(&millimeter!(0.0, 0.0, 8.5)));
        // the surface is not curved along y, so the y position does not matter
        assert!(s.is_behind(&millimeter!(0.0, 100.0, 9.5)));
        assert!(!s.is_behind(&millimeter!(0.0, 100.0, 8.5)));
        // the sag at x = 0.6 mm is 1 - sqrt(1 - 0.36) = 0.2 mm
        assert!(s.is_behind(&millimeter!(0.6, 0.0, 9.3)));
        assert!(!s.is_behind(&millimeter!(0.6, 0.0, 9.1)));
        Ok(())
    }
    #[test]
    fn is_behind_concave() -> OpmResult<()> {
        // negative radius: the axis lies in front of the surface, whose vertex is at z = 11 mm
        let iso = Isometry::new_along_z(millimeter!(10.0))?;
        let s = Cylinder::new(millimeter!(-1.0), iso)?;
        assert!(s.is_behind(&millimeter!(0.0, 0.0, 11.5)));
        assert!(!s.is_behind(&millimeter!(0.0, 0.0, 10.5)));
        Ok(())
    }
    #[test]
    fn intersect_positive_on_axis() -> OpmResult<()> {
        let iso = Isometry::new_along_z(millimeter!(10.0))?;
        let s = Cylinder::new(millimeter!(1.0), iso)?;
        let ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0))?;
        let (intersection_point, normal) = s
            .calc_intersect_and_normal(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_abs_diff_eq!(intersection_point.x.value, 0.0);
        assert_abs_diff_eq!(intersection_point.y.value, 0.0);
        assert_abs_diff_eq!(intersection_point.z.value, 0.009);
        assert_abs_diff_eq!(normal.x, 0.0);
        assert_abs_diff_eq!(normal.y, 0.0);
        assert_abs_diff_eq!(normal.z, -1.0);

        let ray = Ray::new_collimated(millimeter!(0.0, 1.0, 0.0), nanometer!(1053.0), joule!(1.0))?;
        let (intersection_point, normal) = s
            .calc_intersect_and_normal(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_abs_diff_eq!(intersection_point.x.value, 0.0);
        assert_abs_diff_eq!(intersection_point.y.value, 0.001);
        assert_abs_diff_eq!(intersection_point.z.value, 0.009);
        assert_abs_diff_eq!(normal.x, 0.0);
        assert_abs_diff_eq!(normal.y, 0.0);
        assert_abs_diff_eq!(normal.z, -1.0);
        Ok(())
    }
    #[test]
    fn intersect_positive_on_axis_behind() -> OpmResult<()> {
        let ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0))?;
        let iso = Isometry::new_along_z(millimeter!(-10.0))?;
        let s = Cylinder::new(millimeter!(1.0), iso.clone())?;
        assert_eq!(s.calc_intersect_and_normal(&ray), None);
        Ok(())
    }
    #[test]
    fn intersect_positive_collinear_no_intersect() -> OpmResult<()> {
        let iso = Isometry::new_along_z(millimeter!(10.0))?;
        let s = Cylinder::new(millimeter!(1.0), iso)?;
        let ray = Ray::new_collimated(millimeter!(1.1, 0.0, 0.0), nanometer!(1053.0), joule!(1.0))?;
        assert_eq!(s.calc_intersect_and_normal(&ray), None);
        Ok(())
    }
    #[test]
    fn intersect_positive_collinear_touch() -> OpmResult<()> {
        let wvl = nanometer!(1053.0);
        let ray = Ray::new_collimated(millimeter!(1.0, 0.0, 0.0), wvl, joule!(1.0))?;
        let iso = Isometry::new_along_z(millimeter!(10.0))?;
        let s = Cylinder::new(millimeter!(1.0), iso)?;
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((millimeter!(1.0, 0.0, 10.0), Vector3::x()))
        );
        let ray = Ray::new_collimated(millimeter!(-1.0, 0.0, 0.0), wvl, joule!(1.0))?;
        let (intersection_point, normal) = s
            .calc_intersect_and_normal(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
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
        Ok(())
    }
    #[test]
    fn intersect_positive_back_propagating_on_axis() -> OpmResult<()> {
        let wvl = nanometer!(1053.0);
        let ray = Ray::new(millimeter!(0.0, 0.0, 10.0), -Vector3::z(), wvl, joule!(1.0))?;
        let iso = Isometry::new_along_z(millimeter!(0.0))?;
        let s = Cylinder::new(millimeter!(1.0), iso)?;
        let (intersection_point, normal) = s
            .calc_intersect_and_normal(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_abs_diff_eq!(intersection_point.x.value, 0.0);
        assert_abs_diff_eq!(intersection_point.y.value, 0.0);
        assert_abs_diff_eq!(intersection_point.z.value, -0.001);
        assert_abs_diff_eq!(normal.x, 0.0);
        assert_abs_diff_eq!(normal.y, 0.0);
        assert_abs_diff_eq!(normal.z, 1.0);
        Ok(())
    }
    #[test]
    fn intersect_negative_back_propagating_on_axis() -> OpmResult<()> {
        let wvl = nanometer!(1053.0);
        let ray = Ray::new(millimeter!(0.0, 0.0, 10.0), -Vector3::z(), wvl, joule!(1.0))?;
        let iso = Isometry::new_along_z(millimeter!(0.0))?;
        let s = Cylinder::new(millimeter!(-1.0), iso)?;
        let (intersection_point, normal) = s
            .calc_intersect_and_normal(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_abs_diff_eq!(intersection_point.x.value, 0.0);
        assert_abs_diff_eq!(intersection_point.y.value, 0.0);
        assert_abs_diff_eq!(intersection_point.z.value, 0.001);
        assert_abs_diff_eq!(normal.x, 0.0);
        assert_abs_diff_eq!(normal.y, 0.0);
        assert_abs_diff_eq!(normal.z, 1.0);
        Ok(())
    }
    #[test]
    fn intersect_negative_on_axis() -> OpmResult<()> {
        let iso = Isometry::new_along_z(millimeter!(10.0))?;
        let s = Cylinder::new(millimeter!(-1.0), iso)?;
        let ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0))?;
        let (intersection_point, normal) = s
            .calc_intersect_and_normal(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_abs_diff_eq!(intersection_point.x.value, 0.0);
        assert_abs_diff_eq!(intersection_point.y.value, 0.0);
        assert_abs_diff_eq!(intersection_point.z.value, 0.011);
        assert_abs_diff_eq!(normal.x, 0.0);
        assert_abs_diff_eq!(normal.y, 0.0);
        assert_abs_diff_eq!(normal.z, -1.0);

        let ray = Ray::new_collimated(millimeter!(0.0, 1.0, 0.0), nanometer!(1053.0), joule!(1.0))?;
        let (intersection_point, normal) = s
            .calc_intersect_and_normal(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_abs_diff_eq!(intersection_point.x.value, 0.0);
        assert_abs_diff_eq!(intersection_point.y.value, 0.001);
        assert_abs_diff_eq!(intersection_point.z.value, 0.011);
        assert_abs_diff_eq!(normal.x, 0.0);
        assert_abs_diff_eq!(normal.y, 0.0);
        assert_abs_diff_eq!(normal.z, -1.0);
        Ok(())
    }
    #[test]
    fn intersect_positive_back_propagating_off_axis() -> OpmResult<()> {
        let wvl = nanometer!(1053.0);
        // Ray starts at z=10, moves in -z direction, offset in x
        let ray = Ray::new(millimeter!(0.6, 0.0, 10.0), -Vector3::z(), wvl, joule!(1.0))?;

        // Cylinder at origin with radius 1mm
        let iso = Isometry::new_along_z(millimeter!(0.0))?;
        let s = Cylinder::new(millimeter!(1.0), iso)?;

        // Geometry: x^2 + z^2 = r^2 => 0.6^2 + z^2 = 1.0^2
        // 0.36 + z^2 = 1.0 => z^2 = 0.64 => z = -0.8 (first hit from z=-10)
        let (intersection_point, normal) = s
            .calc_intersect_and_normal(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;

        assert_abs_diff_eq!(intersection_point.x.get::<millimeter>(), 0.6);
        assert_abs_diff_eq!(
            intersection_point.z.get::<millimeter>(),
            -0.8,
            epsilon = 1e-12
        );

        // Initial normal would be [0.6, 0.0, 0.8] normalized
        // Because it's convex (radius > 0) and back-propagating (dir.z < 0),
        // Expected normal: [-0.6, 0.0, -0.8]
        assert_abs_diff_eq!(normal.x, -0.6, epsilon = 1e-12);
        assert_abs_diff_eq!(normal.y, 0.0);
        assert_abs_diff_eq!(normal.z, 0.8, epsilon = 1e-12);
        Ok(())
    }
}
