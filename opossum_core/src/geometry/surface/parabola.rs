//! Parabolic surface
//!
//! This module implements a parabolic surface with a given focal length.

use crate::{
    error::{OpmResult, OpossumError},
    light::Ray,
    meter,
};
use nalgebra::{Point2, Point3, Vector3, vector};
use roots::{Roots, find_roots_quadratic};
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use super::GeoSurface;

/// A parabolic surface with a given focal length, its vertex at the origin of its frame.
///
/// **Note**: A positive focal length leads to a parabolic surface with its "opening" towards the
/// positive z axis.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Parabola {
    focal_length: Length,
}

impl Parabola {
    /// Create a new [`Parabola`].
    ///
    /// # Arguments
    ///
    /// * `focal_length` - the focal length; a positive one opens the parabola towards +z.
    ///
    /// # Errors
    ///
    /// This function will return an error if the focal length is 0.0 or not finite.
    pub fn new(focal_length: Length) -> OpmResult<Self> {
        if !focal_length.is_normal() {
            return Err(OpossumError::Other(
                "focal length must be != 0.0 and finite".into(),
            ));
        }
        Ok(Self { focal_length })
    }
    /// Returns the focal length of this [`Parabola`].
    #[must_use]
    pub const fn focal_length(&self) -> Length {
        self.focal_length
    }
}

impl GeoSurface for Parabola {
    #[allow(clippy::suboptimal_flops)] // don't use mul_add here for a,b,c because the current implementation is faster!
    fn calc_intersect_and_normal(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        let dir = ray.direction();
        let pos_vec = ray.position().coords.map(|v| v.value);
        let f_length = self.focal_length.value;
        // parabola formula (at origin)
        // x^2 + y^2 - 4fz = 0
        //
        // insert ray (p: position, d: direction):
        // (p_x+t*d_x)^2 + (p_y+t*d_y)^2 - 4f*(p_z+t*d_z) = 0
        // This translates into the qudratic equation
        // at^2 + bt + c = 0 with
        // a = d_x^2+d_y^2
        // b = 2* (p_x*d_x + p_y*d_y - 2*f*d_z)
        // c = p_x^2 + p_y^2 - 4f*p_z
        let a = dir.x.mul_add(dir.x, dir.y * dir.y);
        let b = 2. * (2. * f_length).mul_add(-dir.z, pos_vec.x.mul_add(dir.x, pos_vec.y * dir.y));
        let c = (4. * f_length).mul_add(
            -pos_vec.z,
            pos_vec.x.mul_add(pos_vec.x, pos_vec.y * pos_vec.y),
        );

        if a.abs() < 1e-9 {
            if b.abs() < 1e-9 {
                return None; // Ray is on the surface and parallel to it. No unique intersection.
            }
            let t = -c / b;
            if t < 0.0 {
                return None; // Intersection is behind the ray.
            }
            let intersection_point = pos_vec + t * dir;
            let normal =
                vector![intersection_point.x, intersection_point.y, -2. * f_length].normalize();
            return Some((
                meter!(
                    intersection_point.x,
                    intersection_point.y,
                    intersection_point.z
                ),
                normal,
            ));
        }
        let real_t = match find_roots_quadratic(a, b, c) {
            Roots::One(t) => t[0],
            Roots::Two(t) => {
                if self.focal_length.is_sign_negative() {
                    // Concave (opens towards -z)
                    f64::max(t[0], t[1])
                } else {
                    // Convex (opens towards +z)
                    f64::min(t[0], t[1])
                }
            }
            _ => return None,
        };
        if real_t < 0.0 {
            return None;
        }
        let intersection_point = pos_vec + real_t * dir;
        let normal_vector =
            vector![intersection_point.x, intersection_point.y, -2. * f_length].normalize();

        Some((
            meter!(
                intersection_point.x,
                intersection_point.y,
                intersection_point.z
            ),
            normal_vector,
        ))
    }

    fn local_z_at(&self, transversal_position: &Point2<Length>) -> Option<Length> {
        // The local surface is x^2 + y^2 = 4*f*z with its vertex in the origin, so its sag at
        // (x, y) is (x^2 + y^2) / (4*f) and it lies above the whole transversal plane. This holds
        // for both signs of the focal length: a negative one simply yields a negative sag, i.e. a
        // surface curving towards -z.
        let (x, y) = (transversal_position.x.value, transversal_position.y.value);
        Some(meter!(x.mul_add(x, y * y) / (4. * self.focal_length.value)))
    }
    fn local_normal_at(&self, transversal_position: &Point2<Length>) -> Option<Vector3<f64>> {
        // The local surface is x^2 + y^2 - 4*f*z = 0, whose gradient (-2x, -2y, 4f) is normal to
        // it — halved here, which changes nothing once it is normalized. That gradient points
        // towards -z for a negative focal length, so the sign of f turns it back.
        let (x, y) = (transversal_position.x.value, transversal_position.y.value);
        let focal_length = self.focal_length.value;
        Some(vector![-x, -y, 2. * focal_length].normalize() * focal_length.signum())
    }
    fn is_behind(&self, point: &Point3<Length>) -> bool {
        self.local_z_at(&Point2::new(point.x, point.y))
            .is_some_and(|sag| point.z >= sag)
    }
    fn reaches(&self, _distance: Length) -> bool {
        true
    }
    fn name(&self) -> &'static str {
        "parabolic"
    }
}

#[cfg(test)]
mod test {
    use super::Parabola;
    use crate::{
        error::{OpmResult, OpossumError},
        geometry::GeoSurface,
        joule,
        light::Ray,
        meter, nanometer,
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::vector;
    #[test]
    fn new() -> OpmResult<()> {
        assert!(Parabola::new(meter!(0.0)).is_err());
        assert!(Parabola::new(meter!(f64::NAN)).is_err());
        assert!(Parabola::new(meter!(f64::INFINITY)).is_err());
        assert!(Parabola::new(meter!(f64::NEG_INFINITY)).is_err());
        assert_eq!(Parabola::new(meter!(1.0))?.focal_length(), meter!(1.0));
        Ok(())
    }
    #[test]
    fn is_behind() -> OpmResult<()> {
        // opening towards +z: the sag at x = 1 m is 1 / (4 * 1 m) = 0.25 m
        let parabola = Parabola::new(meter!(1.0))?;
        assert!(parabola.is_behind(&meter!(0.0, 0.0, 0.1)));
        // a point exactly on the surface counts as behind it
        assert!(parabola.is_behind(&meter!(0.0, 0.0, 0.0)));
        assert!(!parabola.is_behind(&meter!(0.0, 0.0, -0.1)));
        assert!(parabola.is_behind(&meter!(1.0, 0.0, 0.3)));
        assert!(!parabola.is_behind(&meter!(1.0, 0.0, 0.2)));
        assert!(parabola.is_behind(&meter!(0.0, 1.0, 0.3)));
        assert!(!parabola.is_behind(&meter!(0.0, 1.0, 0.2)));
        Ok(())
    }
    #[test]
    fn is_behind_negative_focal_length() -> OpmResult<()> {
        // opening towards -z: the sag at x = 1 m is now -0.25 m
        let parabola = Parabola::new(meter!(-1.0))?;
        assert!(parabola.is_behind(&meter!(0.0, 0.0, 0.1)));
        assert!(!parabola.is_behind(&meter!(0.0, 0.0, -0.1)));
        assert!(parabola.is_behind(&meter!(1.0, 0.0, -0.2)));
        assert!(!parabola.is_behind(&meter!(1.0, 0.0, -0.3)));
        Ok(())
    }
    #[test]
    fn intersect() -> OpmResult<()> {
        let parabola = Parabola::new(meter!(1.0))?;
        let ray = Ray::new_collimated(meter!(-1.0, -1.0, -10.0), nanometer!(1000.0), joule!(1.0))?;
        let (intersection_point, surface_normal) = parabola
            .calc_intersect_and_normal(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_eq!(intersection_point, meter!(-1., -1., 0.5));
        assert_abs_diff_eq!(
            surface_normal,
            vector![
                -0.4082482904638631,
                -0.4082482904638631,
                -0.8164965809277261
            ]
        );
        Ok(())
    }
    #[test]
    fn intersect_ray_through_focus_concave() -> OpmResult<()> {
        let parabola = Parabola::new(meter!(-1.0))?;
        let direction = vector![0.0, 1.0, 1. - 0.25];
        let ray = Ray::new(
            meter!(0.0, 0.0, -1.0),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )?;
        assert!(parabola.calc_intersect_and_normal(&ray).is_some());
        Ok(())
    }
    #[test]
    fn intersect_ray_through_focus_convex() -> OpmResult<()> {
        let parabola = Parabola::new(meter!(1.0))?;
        let direction = vector![0.0, 0.5, 2.];
        let ray = Ray::new(
            meter!(0.0, -0.5, -1.0),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )?;
        assert!(parabola.calc_intersect_and_normal(&ray).is_some());
        Ok(())
    }
    #[test]
    fn intersect_touching() -> OpmResult<()> {
        let parabola = Parabola::new(meter!(1.0))?;
        let direction = vector![0.0, 1.0, 0.0];
        let ray = Ray::new(
            meter!(0.0, -1.0, 0.0),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )?;
        let (i_point, r_point) = parabola
            .calc_intersect_and_normal(&ray)
            .ok_or(OpossumError::Other("no intersect and normal found".into()))?;
        assert_eq!(i_point.x, meter!(0.0));
        assert_eq!(i_point.y, meter!(0.0));
        assert_eq!(i_point.z, meter!(0.0));
        assert_eq!(r_point.normalize(), vector!(0.0, 0.0, -1.0));
        Ok(())
    }
    #[test]
    fn intersect_not() -> OpmResult<()> {
        let parabola = Parabola::new(meter!(1.0))?;
        let direction = vector![0.0, 1.0, 0.0];
        let ray = Ray::new(
            meter!(0.0, -1.0, -1.0),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )?;
        assert!(parabola.calc_intersect_and_normal(&ray).is_none());

        let direction = vector![0.0, 0.0, -1.0];
        let ray = Ray::new(
            meter!(0.0, 0.0, -1.0),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )?;
        assert!(parabola.calc_intersect_and_normal(&ray).is_none());
        Ok(())
    }
}
