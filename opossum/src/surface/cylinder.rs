//! Spherical surface
//!
//! This module implements a cylindrical surface with a given radius of curvature and a given position / alignment in 3D space.
use nalgebra::{vector, Point3, Vector3};
use roots::{find_roots_quadratic, Roots};
use uom::si::f64::Length;

use super::GeoSurface;
use crate::{meter, radian};
use crate::ray::Ray;
use crate::utils::geom_transformation::Isometry;
use crate::{
    error::{OpmResult, OpossumError},
    render::{Color, Render, Renderable, SDF},
};

#[derive(Debug)]
/// A spherical surface with its anchor point on the optical axis.
pub struct Cylinder {
    radius: Length,
    //position of the center of the cylinder
    base_pos: Point3<Length>,
    //direction in which the cylinder is contructed
    dir: Vector3<f64>,
    //isometry of the cylinder that defines its orientation and position
    isometry: Isometry,
}
impl Cylinder {
    /// Create a new [`Cylinder`] located at a given position.
    ///
    /// **Note**: The anchor point is not the center of the cylinder but a point on the cylinder surface.
    ///
    /// # Errors
    ///
    /// This function will return an error if any components of the `pos` are not finite or if the radius is not normal.
    // pub fn new_from_position(radius: Length, pos: Point3<Length>) -> OpmResult<Self> {
    //     if !radius.is_normal() {
    //         return Err(OpossumError::Other(
    //             "radius of curvature must be != 0.0 and finite".into(),
    //         ));
    //     }
    //     let isometry = Isometry::new(
    //         Point3::new(pos.x, pos.y, pos.z + radius),
    //         radian!(0., 0., 0.),
    //     )?;
    //     Ok(Self { radius, isometry })
    // }
    /// Create a new [`Cylinder`] located and oriented by the given [`Isometry`].
    ///
    /// **Note**: The anchor point is not the center of the cylinder but a point on the cylindric surface.
    ///
    /// # Errors
    ///
    /// This function will return an error if any components of the `pos` are not finite or if the radius is not normal.
    pub fn new(radius: Length, isometry: &Isometry, anchor_point: Point3<Length>, dir: Vector3<f64>) -> OpmResult<Self> {
        if !radius.is_normal() {
            return Err(OpossumError::Other(
                "radius of curvature must be != 0.0 and finite".into(),
            ));
        }
        if anchor_point.iter().any(|x| !x.is_finite()) {
            return Err(OpossumError::Other(
                "Position entries must be finite!".into(),
            ));
        }

        let dir = dir.normalize();
        let isometry = Isometry::new_from_view(anchor_point, dir, Vector3::y());
        Ok(Self {
            radius,
            base_pos: anchor_point,
            dir,
            isometry,
        })
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
                    f64::min(t[0], t[1])
                } else {
                    // concave surface => use max t
                    f64::max(t[0], t[1])
                };
                if real_t.is_sign_negative() {
                    // surface behind beam
                    return None;
                }
                pos + real_t * dir
            }
            _ => unreachable!(),
        };
        let mut normal_vector = intersection_point;
        // remove y component
        normal_vector.y = 0.0;
        normal_vector = normal_vector.normalize();
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
    fn set_isometry(&mut self, isometry: &Isometry) {
        self.isometry = isometry.clone();
    }
    fn isometry(&self) -> &Isometry {
        &self.isometry
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{joule, millimeter, nanometer};
    use approx::assert_abs_diff_eq;

    // #[test]
    // fn new() {
    //     let iso = Isometry::new_along_z(millimeter!(1.0)).unwrap();
    //     assert!(Cylinder::new(millimeter!(f64::NAN), &iso).is_err());
    //     assert!(Cylinder::new(millimeter!(f64::INFINITY), &iso).is_err());
    //     assert!(Cylinder::new(millimeter!(f64::NEG_INFINITY), &iso).is_err());

    //     let s = Cylinder::new(millimeter!(2.0), &iso).unwrap();
    //     assert_eq!(s.radius, millimeter!(2.0));
    //     assert_eq!(s.get_pos(), millimeter!(0.0, 0.0, 3.0));

    //     let s = Cylinder::new(millimeter!(-2.0), &iso).unwrap();
    //     assert_eq!(s.radius, millimeter!(-2.0));
    //     assert_eq!(s.get_pos(), millimeter!(0.0, 0.0, -1.0));
    // }
    // #[test]
    // fn intersect_positive_on_axis() {
    //     let s = Cylinder::new_from_position(millimeter!(1.0), millimeter!(0.0, 0.0, 10.0)).unwrap();
    //     let ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0)).unwrap();
    //     let (intersection_point, normal) = s.calc_intersect_and_normal(&ray).unwrap();
    //     assert_abs_diff_eq!(intersection_point.x.value, 0.0);
    //     assert_abs_diff_eq!(intersection_point.y.value, 0.0);
    //     assert_abs_diff_eq!(intersection_point.z.value, 0.01);
    //     assert_abs_diff_eq!(normal.x, 0.0);
    //     assert_abs_diff_eq!(normal.y, 0.0);
    //     assert_abs_diff_eq!(normal.z, -1.0);

    //     let ray = Ray::new_collimated(millimeter!(0.0, 1.0, 0.0), nanometer!(1053.0), joule!(1.0))
    //         .unwrap();
    //     let (intersection_point, normal) = s.calc_intersect_and_normal(&ray).unwrap();
    //     assert_abs_diff_eq!(intersection_point.x.value, 0.0);
    //     assert_abs_diff_eq!(intersection_point.y.value, 0.001);
    //     assert_abs_diff_eq!(intersection_point.z.value, 0.01);
    //     assert_abs_diff_eq!(normal.x, 0.0);
    //     assert_abs_diff_eq!(normal.y, 0.0);
    //     assert_abs_diff_eq!(normal.z, -1.0);
    // }
    // #[test]
    // fn intersect_positive_on_axis_behind() {
    //     let ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0)).unwrap();
    //     let s =
    //         Cylinder::new_from_position(millimeter!(1.0), millimeter!(0.0, 0.0, -10.0)).unwrap();
    //     assert_eq!(s.calc_intersect_and_normal(&ray), None);
    //     let s =
    //         Cylinder::new_from_position(millimeter!(-1.0), millimeter!(0.0, 0.0, -10.0)).unwrap();
    //     assert_eq!(s.calc_intersect_and_normal(&ray), None);
    // }
    // #[test]
    // fn intersect_positive_collinear_no_intersect() {
    //     let s = Cylinder::new_from_position(millimeter!(1.0), millimeter!(0.0, 0.0, 10.0)).unwrap();
    //     let ray = Ray::new_collimated(millimeter!(1.1, 0.0, 0.0), nanometer!(1053.0), joule!(1.0))
    //         .unwrap();
    //     assert_eq!(s.calc_intersect_and_normal(&ray), None);
    // }
    // #[test]
    // fn intersect_positive_collinear_touch() {
    //     let wvl = nanometer!(1053.0);
    //     let ray = Ray::new_collimated(millimeter!(1.0, 0.0, 0.0), wvl, joule!(1.0)).unwrap();
    //     let s = Cylinder::new_from_position(millimeter!(1.0), millimeter!(0.0, 0.0, 10.0)).unwrap();
    //     assert_eq!(
    //         s.calc_intersect_and_normal(&ray),
    //         Some((millimeter!(1.0, 0.0, 11.0), Vector3::x()))
    //     );
    //     let ray = Ray::new_collimated(millimeter!(-1.0, 0.0, 0.0), wvl, joule!(1.0)).unwrap();
    //     let (intersection_point, normal) = s.calc_intersect_and_normal(&ray).unwrap();
    //     assert_eq!(intersection_point.y, Length::zero());
    //     assert_abs_diff_eq!(intersection_point.x.value, -0.001);
    //     assert_abs_diff_eq!(
    //         intersection_point.z.value,
    //         0.011,
    //         epsilon = 1000.0 * f64::EPSILON
    //     );
    //     assert_abs_diff_eq!(normal.x, -1.0);
    //     assert_abs_diff_eq!(normal.y, 0.0);
    //     assert_abs_diff_eq!(normal.z, 0.0);
    // }
    // #[test]
    // fn intersect_negative_on_axis() {
    //     let s =
    //         Cylinder::new_from_position(millimeter!(-1.0), millimeter!(0.0, 0.0, 10.0)).unwrap();
    //     let ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0)).unwrap();
    //     let (intersection_point, normal) = s.calc_intersect_and_normal(&ray).unwrap();
    //     assert_abs_diff_eq!(intersection_point.x.value, 0.0);
    //     assert_abs_diff_eq!(intersection_point.y.value, 0.0);
    //     assert_abs_diff_eq!(intersection_point.z.value, 0.01);
    //     assert_abs_diff_eq!(normal.x, 0.0);
    //     assert_abs_diff_eq!(normal.y, 0.0);
    //     assert_abs_diff_eq!(normal.z, -1.0);
    // }
}


// use approx::relative_eq;
// use nalgebra::{Point3, Vector2, Vector3};
// use roots::find_roots_quadratic;
// use uom::si::{f64::Length, length::millimeter};

// use super::Surface;
// use crate::{
//     error::{OpmResult, OpossumError},
//     millimeter,
//     ray::Ray,
//     render::{Color, Render, Renderable, SDF},
//     utils::geom_transformation::Isometry,
// };
// use roots::Roots;

// #[derive(Debug)]
// /// A spherical surface with its origin on the optical axis.
// pub struct Cylinder {
//     /// length of the cylinder
//     length: Length,
//     ///radius of the cylinder
//     radius: Length,
//     ///position of the center of the cylinder
//     base_pos: Point3<Length>,
//     ///direction in which the cylinder is contructed
//     dir: Vector3<f64>,
//     ///isometry of the cylinder that defines its orientation and position
//     isometry: Isometry,
// }
// impl Cylinder {
//     /// Generate a new [`Cylinder`] surface
//     /// # Attributes
//     /// - `length`: length of the cylinder
//     /// - `radius`: radius of the cylinder
//     /// - `pos`: position of one of the end faces of the cylinder
//     /// - `dir`: direction of the cylinder construction
//     ///
//     /// # Errors
//     ///
//     /// This function will return an error if
//     /// - the radius of curvature is 0.0 or not finite.
//     /// - the construction direction is zero in length or is not finite
//     /// - the length is not finite
//     /// - the center position is not finite
//     pub fn new(
//         length: Length,
//         radius: Length,
//         anchor_point: Point3<Length>,
//         dir: Vector3<f64>,
//     ) -> OpmResult<Self> {
//         if !radius.is_normal() {
//             return Err(OpossumError::Other(
//                 "radius of curvature must be != 0.0 and finite!".into(),
//             ));
//         }
//         if relative_eq!(dir.norm(), 0.) {
//             return Err(OpossumError::Other(
//                 "construction-direction vector must not be zero in length!".into(),
//             ));
//         }
//         if dir.iter().any(|x| !x.is_finite()) {
//             return Err(OpossumError::Other(
//                 "construction-direction vector entries must be finite!".into(),
//             ));
//         }
//         if anchor_point.iter().any(|x| !x.is_finite()) {
//             return Err(OpossumError::Other(
//                 "Position entries must be finite!".into(),
//             ));
//         }
//         if !length.is_finite() {
//             return Err(OpossumError::Other(
//                 "length of cylinder must be finite!".into(),
//             ));
//         }
//         let dir = dir.normalize();
//         let isometry = Isometry::new_from_view(anchor_point, dir, Vector3::y());
//         Ok(Self {
//             length,
//             radius,
//             base_pos: anchor_point,
//             dir,
//             isometry,
//         })
//     }
// }
// impl Surface for Cylinder {
//     fn calc_intersect_and_normal(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
//         let ray_pos = Point3::new(
//             ray.position().x.get::<millimeter>(),
//             ray.position().y.get::<millimeter>(),
//             ray.position().z.get::<millimeter>(),
//         );
//         let base_p_vec = (self.base_pos - ray.position())
//             .iter()
//             .map(uom::si::f64::Length::get::<millimeter>)
//             .collect::<Vec<f64>>();
//         let base_p_vec = Vector3::new(base_p_vec[0], base_p_vec[1], base_p_vec[2]);
//         let a = ray.direction().cross(&self.dir).norm_squared() / 2.;
//         let b = -ray
//             .direction()
//             .cross(&self.dir)
//             .dot(&base_p_vec.cross(&self.dir));
//         let c = -self.radius.get::<millimeter>() * self.radius.get::<millimeter>() / 2.;

//         // Solve t of qudaratic equation
//         let roots = find_roots_quadratic(a, b, c);
//         let (intersection, normal_vec) = {
//             let distance = match roots {
//                 // no intersection
//                 Roots::No(_) => return None,
//                 // "just touching" intersection
//                 Roots::One(t) => {
//                     if t[0] >= 0.0 {
//                         t[0]
//                     } else {
//                         return None;
//                     }
//                 }
//                 // "regular" intersection
//                 Roots::Two(t) => {
//                     let real_t = f64::min(t[0], t[1]);
//                     if real_t.is_sign_negative() {
//                         // surface behind beam
//                         return None;
//                     }
//                     real_t
//                 }
//                 _ => unreachable!(),
//             };
//             let intersection = ray_pos + distance * ray.direction();
//             let signed_distance = self.dir.dot(&(ray.direction() * distance - base_p_vec));
//             let normal =
//                 (ray.direction() * distance - signed_distance * self.dir - base_p_vec).normalize();
//             (intersection, normal)
//         };

//         Some((
//             millimeter!(intersection.x, intersection.y, intersection.z),
//             normal_vec,
//         ))
//     }
//     fn set_isometry(&mut self, isometry: &Isometry) {
//         self.isometry = isometry.clone();
//     }
// }

// impl Color for Cylinder {
//     fn get_color(&self, _p: &Point3<f64>) -> Vector3<f64> {
//         Vector3::<f64>::new(0.6, 0.5, 0.4)
//     }
// }

// impl Renderable<'_> for Cylinder {}
// impl Render<'_> for Cylinder {}

// impl SDF for Cylinder {
//     #[allow(clippy::manual_clamp)]
//     fn sdf_eval_point(&self, p: &Point3<f64>) -> f64 {
//         let p_out = self.isometry.inverse_transform_point_f64(p);
//         let d = Vector2::new(p_out.x.hypot(p_out.y), p_out.z.abs())
//             - Vector2::<f64>::new(self.radius.value, self.length.value / 2.);
//         let d_max = Vector2::new(d.x.max(0.), d.y.max(0.));
//         d.x.max(d.y).min(0.) + d_max.x.hypot(d_max.y)
//     }
// }
