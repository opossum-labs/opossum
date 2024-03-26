use approx::relative_eq;
use nalgebra::{Point3, Vector2, Vector3};
use num::Zero;
use roots::find_roots_quadratic;
use uom::si::{f64::Length, length::millimeter};

use crate::{error::{OpmResult, OpossumError}, millimeter, radian, ray::Ray, signed_distance_function::SDF, utils::geom_transformation::Isometry};
use roots::Roots;
use super::Surface;


#[derive(Debug)]
/// A spherical surface with its origin on the optical axis.
pub struct Cylinder {
    /// length of the cylinder
    length: Length,
    ///radius of the cylinder
    radius: Length,
    ///position of the center of the end face
    base_pos: Point3<Length>,
    ///direction in which the cylinder is contructed
    dir: Vector3<f64>,
    ///isometry of the cylinder that defines its orientation and position
    isometry: Isometry
}
impl Cylinder {
    /// Generate a new [`Cylinder`] surface
    /// # Attributes
    /// - `length`: length of the cylinder
    /// - `radius`: radius of the cylinder
    /// - `pos`: position of one of the end faces of the cylinder
    /// - `dir`: direction of the cylinder construction
    ///
    /// # Errors
    ///
    /// This function will return an error if the radius of curvature is 0.0 or not finite.
    pub fn new(length: Length, radius: Length, base_pos: Point3<Length>,  dir: Vector3<f64>) -> OpmResult<Self> {
        if !radius.is_normal() {
            return Err(OpossumError::Other(
                "radius of curvature must be != 0.0 and finite!".into(),
            ));
        }
        if relative_eq!(dir.norm(), 0.) {
            return Err(OpossumError::Other(
                "construction-direction vector must not be zero in length!".into(),
            ));
        }
        if dir.iter().any(|x| !x.is_finite()){
            return Err(OpossumError::Other(
                "construction-direction vector entries must be finite!".into(),
            ));
        }
        if base_pos.iter().any(|x| !x.is_finite()){
            return Err(OpossumError::Other(
                "Position entries must be finite!".into(),
            ));
        }
        if !length.is_finite(){
            return Err(OpossumError::Other(
                "length of cylinder must be finite!".into(),
            ));
        }
        let dir = dir.normalize();
        let axisangle = radian!(
            dir.dot(&Vector3::x()).acos(),
            dir.dot(&Vector3::y()).acos(),
            dir.dot(&Vector3::z()).acos()
        );
        let isometry = Isometry::new(base_pos, axisangle);
        Ok(Self{
            length,
            radius,
            base_pos,
            dir,
            isometry
        })
    }
}
impl Surface for Cylinder {
    fn calc_intersect_and_normal(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        let ray_pos = Point3::new(ray.position().x.get::<millimeter>(), ray.position().y.get::<millimeter>(), ray.position().z.get::<millimeter>());
        let base_p_vec = (self.base_pos - ray.position()).iter().map(uom::si::f64::Length::get::<millimeter>).collect::<Vec<f64>>();
        let base_p_vec = Vector3::new(base_p_vec[0], base_p_vec[1], base_p_vec[2]);
        let a = ray.direction().cross(&self.dir).norm_squared()/2.;
        let b = -ray.direction().cross(&self.dir).dot(&base_p_vec.cross(&self.dir));
        let c = -self.radius.get::<millimeter>()*self.radius.get::<millimeter>()/2.;

        // Solve t of qudaratic equation
        let roots = find_roots_quadratic(a, b, c);
        let (intersection, normal_vec) = {
            let distance = match roots {
                // no intersection
                Roots::No(_) => return None,
                // "just touching" intersection
                Roots::One(t) => {
                    if t[0] >= 0.0 {
                         t[0]
                    } else {
                        return None;
                    }
                }
                // "regular" intersection
                Roots::Two(t) => {
                    let real_t = f64::min(t[0], t[1]);
                    if real_t.is_sign_negative() {
                        // surface behind beam
                        return None;
                    }
                    real_t
                }
                _ => unreachable!(),
            };
            let intersection = (ray_pos + distance * ray.direction());
            let signed_distance = self.dir.dot(&(ray.direction()*distance-base_p_vec));
            let normal =(ray.direction()*distance- signed_distance*self.dir- base_p_vec).normalize();
            (intersection, normal)
        };

        Some((
            millimeter!(
                intersection.x,
                intersection.y,
                intersection.z
            ),
            normal_vec,
        ))
    }
}

impl SDF for Cylinder{
    fn eval_point(&self, p: &Point3<Length>) -> Length {
        let p = self.isometry.inverse_transform_point(&p);
        let d = Vector2::new((p.x*p.x+p.z*p.z).sqrt() ,p.y.abs()) - Vector2::new(self.radius, self.length);
        d.x.max(d.y).min(Length::zero()) + (d.x*d.x+d.y*d.y).sqrt().max(Length::zero())
    }
}
