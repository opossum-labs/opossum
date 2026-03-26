//! Parabolic surface
//!
//! This module implements a parabolic surface with a given focal length and a given z position on the optical axis.

use crate::{
    error::{OpmResult, OpossumError},
    meter,
    utils::geom_transformation::Isometry,
};
use nalgebra::{Point3, Vector3, vector};
use roots::{Roots, find_roots_quadratic};
use uom::si::f64::Length;

use super::geo_surface::GeoSurface;

#[derive(Clone, Debug)]
/// A parabolic surface with a given focal length and a given z position on the optical axis.
pub struct Parabola {
    focal_length: Length,
    isometry: Isometry,
}

impl Parabola {
    /// Create a new [`Parabola`] located and oriented by the given [`Isometry`].
    ///
    /// **Note**: A positive focal length leads to a parabolic surface with its "opening" towards the positive z axis.
    ///
    /// # Errors
    ///
    /// This function will return an error if the focal length is 0.0 or not finite.
    pub fn new(focal_length: Length, isometry: Isometry) -> OpmResult<Self> {
        if !focal_length.is_normal() {
            return Err(OpossumError::Other(
                "focal length must be != 0.0 and finite".into(),
            ));
        }
        Ok(Self {
            focal_length,
            isometry,
        })
    }
}

impl GeoSurface for Parabola {
    #[allow(clippy::suboptimal_flops)] // don't use mul_add here for a,b,c because the current implementation is faster!
    fn calc_intersect_and_normal_do(
        &self,
        ray: &crate::light::Ray,
    ) -> Option<(Point3<Length>, Vector3<f64>)> {
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
        let roots = find_roots_quadratic(a, b, c);
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
                if self.focal_length.is_sign_negative() {
                    // Concave (opens towards -z)
                    f64::max(t[0], t[1])
                } else {
                    // Convex (opens towards +z)
                    f64::min(t[0], t[1])
                }
            }
            _ => unreachable!(),
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

    fn isometry(&self) -> &Isometry {
        &self.isometry
    }
    fn set_isometry(&mut self, isometry: Isometry) {
        self.isometry = isometry;
    }

    fn name(&self) -> String {
        "parabolic".into()
    }
}

#[cfg(test)]
mod test {
    use super::Parabola;
    use crate::{
        geometry::geo_surface::GeoSurface, joule, light::Ray, meter, nanometer,
        utils::geom_transformation::Isometry,
    };
    use approx::assert_abs_diff_eq;
    use core::f64;
    use nalgebra::vector;
    #[test]
    fn new() {
        assert!(Parabola::new(meter!(0.0), Isometry::identity()).is_err());
        assert!(Parabola::new(meter!(f64::NAN), Isometry::identity()).is_err());
        assert!(Parabola::new(meter!(f64::INFINITY), Isometry::identity()).is_err());
        assert!(Parabola::new(meter!(f64::NEG_INFINITY), Isometry::identity()).is_err());
    }
    #[test]
    fn intersect() {
        let parabola = Parabola::new(meter!(1.0), Isometry::identity()).unwrap();
        let ray = Ray::new_collimated(meter!(-1.0, -1.0, -10.0), nanometer!(1000.0), joule!(1.0))
            .unwrap();
        let (intersection_point, surface_normal) =
            parabola.calc_intersect_and_normal_do(&ray).unwrap();
        assert_eq!(intersection_point, meter!(-1., -1., 0.5));
        assert_abs_diff_eq!(
            surface_normal,
            vector![
                -0.4082482904638631,
                -0.4082482904638631,
                -0.8164965809277261
            ]
        );
    }
    #[test]
    fn intersect_ray_through_focus_concave() {
        let parabola = Parabola::new(meter!(-1.0), Isometry::identity()).unwrap();
        let direction = vector![0.0, 1.0, 1. - 0.25];
        let ray = Ray::new(
            meter!(0.0, 0.0, -1.0),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )
        .unwrap();
        assert!(parabola.calc_intersect_and_normal_do(&ray).is_some());
    }
    // #[test]
    // fn intersect_ray_through_focus_convex() {
    //     let parabola = Parabola::new(meter!(1.0), Isometry::identity()).unwrap();
    //     let direction = vector![0.0, 0.05, -1.];
    //     let ray = Ray::new(
    //         meter!(0.0, 0.0, 1.0),
    //         direction,
    //         nanometer!(1000.0),
    //         joule!(1.0),
    //     )
    //     .unwrap();
    //     assert!(parabola.calc_intersect_and_normal_do(&ray).is_some());
    // }
    #[test]
    fn intersect_ray_through_focus_convex() {
        let parabola = Parabola::new(meter!(1.0), Isometry::identity()).unwrap();
        let direction = vector![0.0, 0.5, 2.];
        let ray = Ray::new(
            meter!(0.0, -0.5, -1.0),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )
        .unwrap();
        assert!(parabola.calc_intersect_and_normal_do(&ray).is_some());
    }
    #[test]
    fn intersect_touching() {
        let parabola = Parabola::new(meter!(1.0), Isometry::identity()).unwrap();
        let direction = vector![0.0, 1.0, 0.0];
        let ray = Ray::new(
            meter!(0.0, -1.0, 0.0),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )
        .unwrap();
        let (i_point, r_point) = parabola.calc_intersect_and_normal_do(&ray).unwrap();
        assert_eq!(i_point.x, meter!(0.0));
        assert_eq!(i_point.y, meter!(0.0));
        assert_eq!(i_point.z, meter!(0.0));
        assert_eq!(r_point.normalize(), vector!(0.0, 0.0, -1.0));
    }
    #[test]
    fn intersect_not() {
        let parabola = Parabola::new(meter!(1.0), Isometry::identity()).unwrap();
        let direction = vector![0.0, 1.0, 0.0];
        let ray = Ray::new(
            meter!(0.0, -1.0, -1.0),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )
        .unwrap();
        assert!(parabola.calc_intersect_and_normal_do(&ray).is_none());

        let direction = vector![0.0, 0.0, -1.0];
        let ray = Ray::new(
            meter!(0.0, 0.0, -1.0),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )
        .unwrap();
        assert!(parabola.calc_intersect_and_normal_do(&ray).is_none());
    }
    #[test]
    fn isometry() {
        let parabola = Parabola::new(meter!(1.0), Isometry::identity()).unwrap();
        assert_eq!(
            parabola.isometry(),
            &Isometry::new_along_z(meter!(0.0)).unwrap()
        );
    }
    #[test]
    fn set_isometry() {
        let mut parabola = Parabola::new(meter!(1.0), Isometry::identity()).unwrap();
        parabola.set_isometry(Isometry::new_along_z(meter!(0.5)).unwrap());
        assert_eq!(
            parabola.isometry(),
            &Isometry::new_along_z(meter!(0.5)).unwrap()
        );
    }
}
