use std::f64::consts::PI;

use approx::relative_eq;
use delaunator::Point;
use nalgebra::{Point3, Vector2, Vector3};
use num::{Float, Zero};
use roots::find_roots_quadratic;
use uom::si::{f64::Length, length::millimeter};

use super::Surface;
use crate::{
    error::{OpmResult, OpossumError}, millimeter, radian, ray::Ray, render::{Render, SDF}, utils::geom_transformation::Isometry
};
use roots::Roots;

#[derive(Debug)]
/// A spherical surface with its origin on the optical axis.
pub struct Cuboid {
    /// length of the cylinder
    length: Point3<Length>,
    ///position of the center of the end face
    base_pos: Point3<Length>,
    ///isometry of the cylinder that defines its orientation and position
    isometry: Isometry,
}
impl Cuboid {
    /// Generate a new [`Cuboid`] surface
    /// # Attributes
    /// - `length`: length of the cuboid in x,y and z direction
    /// - `pos`: position of one of the end faces of the cuboid
    /// - `dir`: direction of the rotated z axis of the cuboid
    ///
    /// # Errors
    ///
    /// This function will return an error if 
    /// - the construction direction is zero in length or is not finite
    /// - the length is not finite
    /// - the center position is not finite
    pub fn new(
        length: Point3<Length>,
        center_pos: Point3<Length>,
        dir: Vector3<f64>
    ) -> OpmResult<Self> {
        if center_pos.iter().any(|x| !x.is_finite()) {
            return Err(OpossumError::Other(
                "Position entries must be finite!".into(),
            ));
        }
        if relative_eq!(dir.norm(), 0.) {
            return Err(OpossumError::Other(
                "construction-direction vector must not be zero in length!".into(),
            ));
        }
        if dir.iter().any(|x| !x.is_finite()) {
            return Err(OpossumError::Other(
                "construction-direction vector entries must be finite!".into(),
            ));
        }
        if length.iter().any(|x| !x.is_finite()) {
            return Err(OpossumError::Other(
                "cube side lengths must be finite!".into(),
            ));
        }

        let isometry = Isometry::new_from_view(center_pos, dir, Vector3::y());
        Ok(Self {
            length,
            base_pos: center_pos,
            isometry
        })
    }
}
impl Render for Cuboid{}
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
//             let intersection = (ray_pos + distance * ray.direction());
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
// }

impl SDF for Cuboid 
{
    fn sdf_eval_point(&self, p: &Point3<f64>) -> f64 {
        let p = self.isometry.inverse_transform_point_f64(p);
        let q = Vector3::new(
            p.x.abs() - self.length.x.value/2.,
            p.y.abs() - self.length.y.value/2.,
            p.z.abs() - self.length.z.value/2.);
        let mut q_max = q.clone();
        q_max.iter_mut().for_each(|x:&mut f64|            *x = x.max(0.0)) ;
        (q_max.x*q_max.x + q_max.y*q_max.y + q_max.z*q_max.z).sqrt() + q.y.max(q.z).max(q.x).min(0.0)
    }
}
