use approx::relative_eq;
use nalgebra::{Point3, Vector2, Vector3};
use roots::find_roots_quadratic;
use uom::si::{f64::Length, length::millimeter};

use super::Surface;
use crate::{
    error::{OpmResult, OpossumError},
    millimeter,
    ray::Ray,
    render::{Color, Render, Renderable, SDF},
    utils::geom_transformation::Isometry,
};
use roots::Roots;
#[derive(Debug)]
/// A spherical surface with its origin on the optical axis.
pub struct Cylinder {
    /// length of the cylinder
    length: Length,
    ///radius of the cylinder
    radius: Length,
    ///position of the center of the cylinder
    base_pos: Point3<Length>,
    ///direction in which the cylinder is contructed
    dir: Vector3<f64>,
    ///isometry of the cylinder that defines its orientation and position
    isometry: Isometry,
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
    /// This function will return an error if
    /// - the radius of curvature is 0.0 or not finite.
    /// - the construction direction is zero in length or is not finite
    /// - the length is not finite
    /// - the center position is not finite
    pub fn new(
        length: Length,
        radius: Length,
        anchor_point: Point3<Length>,
        dir: Vector3<f64>,
    ) -> OpmResult<Self> {
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
        if dir.iter().any(|x| !x.is_finite()) {
            return Err(OpossumError::Other(
                "construction-direction vector entries must be finite!".into(),
            ));
        }
        if anchor_point.iter().any(|x| !x.is_finite()) {
            return Err(OpossumError::Other(
                "Position entries must be finite!".into(),
            ));
        }
        if !length.is_finite() {
            return Err(OpossumError::Other(
                "length of cylinder must be finite!".into(),
            ));
        }
        let dir = dir.normalize();
        let isometry = Isometry::new_from_view(anchor_point, dir, Vector3::y());
        Ok(Self {
            length,
            radius,
            base_pos: anchor_point,
            dir,
            isometry,
        })
    }
}
impl Surface for Cylinder {
    fn calc_intersect_and_normal(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        let ray_pos = Point3::new(
            ray.position().x.get::<millimeter>(),
            ray.position().y.get::<millimeter>(),
            ray.position().z.get::<millimeter>(),
        );
        let base_p_vec = (self.base_pos - ray.position())
            .iter()
            .map(uom::si::f64::Length::get::<millimeter>)
            .collect::<Vec<f64>>();
        let base_p_vec = Vector3::new(base_p_vec[0], base_p_vec[1], base_p_vec[2]);
        let a = ray.direction().cross(&self.dir).norm_squared() / 2.;
        let b = -ray
            .direction()
            .cross(&self.dir)
            .dot(&base_p_vec.cross(&self.dir));
        let c = -self.radius.get::<millimeter>() * self.radius.get::<millimeter>() / 2.;

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
            let intersection = ray_pos + distance * ray.direction();
            let signed_distance = self.dir.dot(&(ray.direction() * distance - base_p_vec));
            let normal =
                (ray.direction() * distance - signed_distance * self.dir - base_p_vec).normalize();
            (intersection, normal)
        };

        Some((
            millimeter!(intersection.x, intersection.y, intersection.z),
            normal_vec,
        ))
    }
    fn set_isometry(&mut self, isometry: &Isometry) {
        self.isometry = isometry.clone();
    }
}

impl Color for Cylinder {
    fn get_color(&self, _p: &Point3<f64>) -> Vector3<f64> {
        Vector3::<f64>::new(0.6, 0.5, 0.4)
    }
}

impl Renderable<'_> for Cylinder {}
impl Render<'_> for Cylinder {}

impl SDF for Cylinder {
    #[allow(clippy::manual_clamp)]
    fn sdf_eval_point(&self, p: &Point3<f64>) -> f64 {
        let p_out = self.isometry.inverse_transform_point_f64(p);
        let d = Vector2::new(p_out.x.hypot(p_out.y), p_out.z.abs())
            - Vector2::<f64>::new(self.radius.value, self.length.value / 2.);
        let d_max = Vector2::new(d.x.max(0.), d.y.max(0.));
        d.x.max(d.y).min(0.) + d_max.x.hypot(d_max.y)
    }
}
