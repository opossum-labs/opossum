//! Spherical surface
//!
//! This module implements a spherical surface with a given radius of curvature.
use super::{GeoSurface, curved_local_normal, curved_local_z, is_behind_curvature};
use crate::{
    error::{OpmResult, OpossumError},
    light::Ray,
    meter,
};
use nalgebra::{Point2, Point3, Vector2, Vector3};
use roots::{Roots, find_roots_quadratic};
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

/// A spherical surface with its vertex at the origin of its frame and its center of curvature one
/// radius along the z axis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sphere {
    radius: Length,
}
impl Sphere {
    /// Create a new [`Sphere`].
    ///
    /// # Arguments
    ///
    /// * `radius` - the radius of curvature. A positive radius has its center behind the vertex
    ///   (+z).
    ///
    /// # Errors
    ///
    /// This function will return an error if the radius is zero or not finite.
    pub fn new(radius: Length) -> OpmResult<Self> {
        if !radius.is_normal() {
            return Err(OpossumError::Other(
                "radius of curvature must be != 0.0 and finite".into(),
            ));
        }
        Ok(Self { radius })
    }
    /// Returns the radius of curvature of this [`Sphere`].
    #[must_use]
    pub const fn radius(&self) -> Length {
        self.radius
    }
    /// The center of curvature, in the vertex frame, in meter.
    const fn center(self) -> Vector3<f64> {
        Vector3::new(0.0, 0.0, self.radius.value)
    }
}
impl GeoSurface for Sphere {
    #[allow(clippy::suboptimal_flops)] // don't use mul_add here for a,b,c because the current implementation is faster!
    fn calc_intersect_and_normal(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        let dir = ray.direction();
        // The sphere formula is stated around the center of curvature.
        let pos_vec = ray.position().coords.map(|v| v.value) - self.center();
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
        let b = 2.0 * dir.dot(&pos_vec);
        let c = pos_vec.norm_squared() - radius * radius;

        let real_t = match find_roots_quadratic(a, b, c) {
            Roots::One(t) => t[0],
            Roots::Two(t) => {
                // A convex surface is met at the near root going forward, a concave one at the far
                // root; going backward it is the other way round.
                if self.radius.is_sign_positive() == is_back_propagating {
                    f64::max(t[0], t[1])
                } else {
                    f64::min(t[0], t[1])
                }
            }
            _ => return None,
        };

        if real_t.is_sign_negative() {
            return None;
        }

        let intersection_point = pos_vec + real_t * dir;
        let mut normal = intersection_point.normalize();
        // The normal always faces the incoming ray.
        if self.radius.is_sign_positive() == is_back_propagating {
            normal *= -1.0;
        }
        let intersection_point = intersection_point + self.center();
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
        // The sphere curves in every transversal direction alike, so only the distance from its
        // axis matters.
        let distance_from_axis = transversal_position
            .x
            .value
            .hypot(transversal_position.y.value);
        curved_local_z(distance_from_axis, self.radius.value).map(|z| meter!(z))
    }
    fn local_normal_at(&self, transversal_position: &Point2<Length>) -> Option<Vector3<f64>> {
        // The sphere curves in every transversal direction alike, so both components tilt the
        // normal.
        curved_local_normal(
            Vector2::new(transversal_position.x.value, transversal_position.y.value),
            self.radius.value,
        )
    }
    fn is_behind(&self, point: &Point3<Length>) -> bool {
        // The surface is the ball of |radius| around the center of curvature.
        let from_center = point.coords.map(|c| c.value) - self.center();
        is_behind_curvature(from_center.norm(), self.radius.value)
    }
    fn reaches(&self, distance: Length) -> bool {
        distance <= self.radius.abs()
    }
    fn name(&self) -> &'static str {
        "sphere"
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        geometry::{Face, SurfaceShape},
        joule, millimeter, nanometer,
        utils::geom_transformation::Isometry,
    };
    use approx::assert_abs_diff_eq;
    use num_traits::Zero;

    /// A sphere of the given radius of curvature with its vertex at the given position.
    fn sphere_at(radius: Length, vertex: Point3<Length>) -> OpmResult<Face> {
        Ok(Face::new(
            SurfaceShape::Sphere(Sphere::new(radius)?),
            Isometry::new_translation(vertex)?,
        ))
    }

    #[test]
    fn is_behind_convex() -> OpmResult<()> {
        // vertex at z = 0, center of curvature at z = +50 mm
        let s = sphere_at(millimeter!(50.0), millimeter!(0.0, 0.0, 0.0))?;
        assert!(s.is_behind(&millimeter!(0.0, 0.0, 1.0)));
        // a point exactly on the surface counts as behind it
        assert!(s.is_behind(&millimeter!(0.0, 0.0, 0.0)));
        assert!(!s.is_behind(&millimeter!(0.0, 0.0, -1.0)));
        // the sag at x = 10 mm is 50 - sqrt(50^2 - 10^2) = 1.0102 mm
        assert!(s.is_behind(&millimeter!(10.0, 0.0, 1.1)));
        assert!(!s.is_behind(&millimeter!(10.0, 0.0, 0.9)));
        Ok(())
    }
    #[test]
    fn is_behind_concave() -> OpmResult<()> {
        // vertex at z = 0, center of curvature at z = -50 mm
        let s = sphere_at(millimeter!(-50.0), millimeter!(0.0, 0.0, 0.0))?;
        assert!(s.is_behind(&millimeter!(0.0, 0.0, 1.0)));
        assert!(s.is_behind(&millimeter!(0.0, 0.0, 0.0)));
        assert!(!s.is_behind(&millimeter!(0.0, 0.0, -1.0)));
        // the sag at x = 10 mm now points towards -z
        assert!(s.is_behind(&millimeter!(10.0, 0.0, -0.9)));
        assert!(!s.is_behind(&millimeter!(10.0, 0.0, -1.1)));
        Ok(())
    }
    /// The surface lies at z = 0 at its vertex and at its sag further out, up to its radius.
    #[test]
    fn local_z_at_is_the_sag() -> OpmResult<()> {
        for radius in [50.0, -50.0] {
            let s = Sphere::new(millimeter!(radius))?;
            assert_eq!(s.local_z_at(&millimeter!(0.0, 0.0)), Some(Length::zero()));
            let sag = s
                .local_z_at(&millimeter!(6.0, 8.0))
                .ok_or_else(|| OpossumError::Other("no sag".into()))?;
            let expected = radius.signum() * (50.0 - 50.0_f64.mul_add(50.0, -100.0).sqrt());
            assert_abs_diff_eq!(sag.value, millimeter!(expected).value, epsilon = 1e-15);
            assert!(s.local_z_at(&millimeter!(50.1, 0.0)).is_none());
        }
        Ok(())
    }
    #[test]
    fn new() -> OpmResult<()> {
        assert!(Sphere::new(millimeter!(f64::NAN)).is_err());
        assert!(Sphere::new(millimeter!(f64::INFINITY)).is_err());
        assert!(Sphere::new(millimeter!(f64::NEG_INFINITY)).is_err());
        assert!(Sphere::new(millimeter!(0.0)).is_err());
        assert_eq!(Sphere::new(millimeter!(2.0))?.radius(), millimeter!(2.0));
        Ok(())
    }
    #[test]
    fn intersect_positive_on_axis_forward() -> OpmResult<()> {
        let sphere_position = millimeter!(0.0, 0.0, 0.0);
        let s = sphere_at(millimeter!(10.0), sphere_position)?;

        // start "within" the sphere (not really)...
        let ray =
            Ray::new_collimated(millimeter!(0.0, 0.0, -5.0), nanometer!(1053.0), joule!(1.0))?;
        let (intersection_point, normal) = s
            .intersect(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_eq!(intersection_point, sphere_position);
        assert_eq!(normal, -Vector3::z());

        // start "outside" the sphere
        let ray = Ray::new_collimated(
            millimeter!(0.0, 0.0, -15.0),
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        let (intersection_point, _) = s
            .intersect(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_abs_diff_eq!(intersection_point.x.value, sphere_position.x.value);
        assert_abs_diff_eq!(intersection_point.y.value, sphere_position.y.value);
        assert_abs_diff_eq!(intersection_point.z.value, sphere_position.z.value);

        // non-intersecting
        let ray = Ray::new_collimated(millimeter!(0.0, 0.0, 5.0), nanometer!(1053.0), joule!(1.0))?;
        assert!(s.intersect(&ray).is_none());
        let ray =
            Ray::new_collimated(millimeter!(0.0, 0.0, 15.0), nanometer!(1053.0), joule!(1.0))?;
        assert!(s.intersect(&ray).is_none());
        Ok(())
    }
    #[test]
    fn intersect_negative_on_axis_forward() -> OpmResult<()> {
        let sphere_position = millimeter!(0.0, 0.0, 0.0);
        let s = sphere_at(millimeter!(-10.0), sphere_position)?;

        // start "within" the sphere (not really)...
        let ray =
            Ray::new_collimated(millimeter!(0.0, 0.0, -5.0), nanometer!(1053.0), joule!(1.0))?;
        let (intersection_point, normal) = s
            .intersect(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_eq!(intersection_point, sphere_position);
        assert_eq!(normal, -Vector3::z());

        // start "outside" the sphere
        let ray = Ray::new_collimated(
            millimeter!(0.0, 0.0, -15.0),
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        let (intersection_point, _) = s
            .intersect(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_abs_diff_eq!(intersection_point.x.value, sphere_position.x.value);
        assert_abs_diff_eq!(intersection_point.y.value, sphere_position.y.value);
        assert_abs_diff_eq!(intersection_point.z.value, sphere_position.z.value);

        // non-intersecting
        let ray = Ray::new_collimated(millimeter!(0.0, 0.0, 5.0), nanometer!(1053.0), joule!(1.0))?;
        assert!(s.intersect(&ray).is_none());
        let ray =
            Ray::new_collimated(millimeter!(0.0, 0.0, 15.0), nanometer!(1053.0), joule!(1.0))?;
        assert!(s.intersect(&ray).is_none());
        Ok(())
    }
    #[test]
    fn intersect_positive_on_axis_backward() -> OpmResult<()> {
        let sphere_position = millimeter!(0.0, 0.0, 0.0);
        let s = sphere_at(millimeter!(10.0), sphere_position)?;

        // start "within" the sphere (not really)...
        let ray = Ray::new(
            millimeter!(0.0, 0.0, 5.0),
            -Vector3::z(),
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        let (intersection_point, normal) = s
            .intersect(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_eq!(intersection_point, sphere_position);
        assert_eq!(normal, Vector3::z());

        // start "outside" the sphere
        let ray = Ray::new(
            millimeter!(0.0, 0.0, 15.0),
            -Vector3::z(),
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        let (intersection_point, _) = s
            .intersect(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_abs_diff_eq!(intersection_point.x.value, sphere_position.x.value);
        assert_abs_diff_eq!(intersection_point.y.value, sphere_position.y.value);
        assert_abs_diff_eq!(intersection_point.z.value, sphere_position.z.value);

        // non-intersecting
        let ray = Ray::new(
            millimeter!(0.0, 0.0, -5.0),
            -Vector3::z(),
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        assert!(s.intersect(&ray).is_none());
        let ray = Ray::new(
            millimeter!(0.0, 0.0, -15.0),
            -Vector3::z(),
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        assert!(s.intersect(&ray).is_none());
        Ok(())
    }
    #[test]
    fn intersect_negative_on_axis_backward() -> OpmResult<()> {
        let sphere_position = millimeter!(0.0, 0.0, 0.0);
        let s = sphere_at(millimeter!(-10.0), sphere_position)?;

        // start "within" the sphere (not really)...
        let ray = Ray::new(
            millimeter!(0.0, 0.0, 5.0),
            -Vector3::z(),
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        let (intersection_point, normal) = s
            .intersect(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_eq!(intersection_point, sphere_position);
        assert_eq!(normal, Vector3::z());

        // start "outside" the sphere
        let ray = Ray::new(
            millimeter!(0.0, 0.0, 15.0),
            -Vector3::z(),
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        let (intersection_point, _) = s
            .intersect(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_abs_diff_eq!(intersection_point.x.value, sphere_position.x.value);
        assert_abs_diff_eq!(intersection_point.y.value, sphere_position.y.value);
        assert_abs_diff_eq!(intersection_point.z.value, sphere_position.z.value);

        // non-intersecting
        let ray = Ray::new(
            millimeter!(0.0, 0.0, -5.0),
            -Vector3::z(),
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        assert!(s.intersect(&ray).is_none());
        let ray = Ray::new(
            millimeter!(0.0, 0.0, -15.0),
            -Vector3::z(),
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        assert!(s.intersect(&ray).is_none());
        Ok(())
    }
    #[test]
    fn intersect_positive_collinear_no_intersect() -> OpmResult<()> {
        let ray = Ray::new_collimated(millimeter!(0.0, 1.1, 0.0), nanometer!(1053.0), joule!(1.0))?;
        let s = sphere_at(millimeter!(1.0), millimeter!(0.0, 0.0, 10.0))?;
        assert!(s.intersect(&ray).is_none());
        let ray =
            Ray::new_collimated(millimeter!(0.0, -1.1, 0.0), nanometer!(1053.0), joule!(1.0))?;
        assert!(s.intersect(&ray).is_none());
        Ok(())
    }
    #[test]
    fn intersect_positive_collinear_touch() -> OpmResult<()> {
        let wvl = nanometer!(1053.0);
        let ray = Ray::new_collimated(millimeter!(0.0, 1.0, 0.0), wvl, joule!(1.0))?;
        let s = sphere_at(millimeter!(1.0), millimeter!(0.0, 0.0, 10.0))?;
        assert_eq!(
            s.intersect(&ray),
            Some((millimeter!(0.0, 1.0, 11.0), Vector3::y()))
        );
        let ray = Ray::new_collimated(millimeter!(0.0, -1.0, -1.0), wvl, joule!(1.0))?;
        let (intersection_point, normal) = s
            .intersect(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_eq!(intersection_point.x, Length::zero());
        assert_abs_diff_eq!(intersection_point.y.value, -0.001);
        assert_abs_diff_eq!(
            intersection_point.z.value,
            0.011,
            epsilon = 1000.0 * f64::EPSILON
        );
        assert_abs_diff_eq!(normal, -Vector3::y());
        Ok(())
    }
}
