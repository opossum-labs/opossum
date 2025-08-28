use approx::relative_eq;
use nalgebra::{Point3, Vector3};
use uom::si::f64::Length;

use crate::{
    error::{OpmResult, OpossumError},
    render::{Color, Render, Renderable, SDF},
    utils::geom_transformation::Isometry,
};

#[derive(Debug)]
/// A spherical surface with its origin on the optical axis.
pub struct Cuboid {
    /// length of the cylinder
    length: Point3<Length>,
    ///position of the center of the cuboid
    anchor_point: Point3<Length>,
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
        dir: Vector3<f64>,
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
            anchor_point: center_pos,
            isometry,
        })
    }

    /// Returns the anchor point of thie [`Cuboid`]
    #[must_use]
    pub const fn get_anchor_point(&self) -> Point3<Length> {
        self.anchor_point
    }
}

impl Color for Cuboid {
    fn get_color(&self, _p: &Point3<f64>) -> Vector3<f64> {
        Vector3::<f64>::new(0.7, 0.6, 0.5)
    }
}
impl Renderable<'_> for Cuboid {}
impl Render<'_> for Cuboid {}

impl SDF for Cuboid {
    fn sdf_eval_point(&self, p: &Point3<f64>) -> f64 {
        let p_out = self.isometry.inverse_transform_point_f64(p);
        let q = Vector3::new(
            p_out.x.abs() - self.length.x.value / 2.,
            p_out.y.abs() - self.length.y.value / 2.,
            p_out.z.abs() - self.length.z.value / 2.,
        );
        let mut q_max = q;
        q_max.iter_mut().for_each(|x: &mut f64| *x = x.max(0.0));
        q_max
            .x
            .mul_add(q_max.x, q_max.y.mul_add(q_max.y, q_max.z.powi(2)))
            .sqrt()
            + q.y.max(q.z).clamp(q.x, 0.0) //q.y.max(q.z).max(q.x).min(0.0)
    }
}
