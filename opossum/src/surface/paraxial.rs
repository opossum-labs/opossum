//! Paraxial surface (ideal lens)
//!
//! This module implements a paraxial surface with a given focal length. The geometric shape correpsonds to a flat surface but
//! the refraction corresponds to a perfect lens.
use super::geo_surface::GeoSurface;
use crate::{
    error::{OpmResult, OpossumError},
    meter, millimeter, radian,
    ray::Ray,
    utils::geom_transformation::Isometry,
};
use nalgebra::{Point3, Vector3};
use num::Zero;
use uom::si::f64::Length;

#[derive(Debug, Clone)]
/// A paraxial surface with a given focal length.
pub struct Paraxial {
    focal_length: Length,
    isometry: Isometry,
}

impl Default for Paraxial {
    /// Create a paraxial surface located at the origin on the x/y plane with a focal length of 100 mm (focussing).
    fn default() -> Self {
        Self {
            focal_length: millimeter!(100.0),
            isometry: Isometry::identity(),
        }
    }
}

impl Paraxial {
    /// Create an new [`Paraxial`] surface (perfect thin lens) with a given focal length.
    ///
    /// A positive focal length corresponds to a focussing lens.
    ///
    /// # Errors
    ///
    /// This function will return an error if the focal length is zero or not finite.
    pub fn new(focal_length: Length, iso: Isometry) -> OpmResult<Self> {
        if !focal_length.is_normal() {
            return Err(OpossumError::Other(
                "focal length must be != 0 and finite.".into(),
            ));
        }
        Ok(Self {
            focal_length,
            isometry: iso,
        })
    }
    /// Create a new [`Paraxial`] surface at the given position with a given focal length.
    ///
    /// The surface is aligned parallel to the x/y plane. A positive focal length corresponds
    /// to a focussing lens.
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn new_at_position(pos: Point3<Length>, focal_length: Length) -> OpmResult<Self> {
        let isometry = Isometry::new(pos, radian!(0., 0., 0.))?;
        Self::new(focal_length, isometry)
    }
}
impl GeoSurface for Paraxial {
    fn calc_intersect_and_normal_do(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        let mut trans_pos_in_m = ray.position().map(|c| c.value);
        let trans_dir = ray.direction();
        // Check, if ray position is on the surface, then directly return position as intersection point
        if !trans_pos_in_m.z.is_zero() {
            let distance_in_z_direction = -trans_pos_in_m.z;
            if distance_in_z_direction.signum() != trans_dir.z.signum() {
                // Ray propagates away from the plane => no intersection
                return None;
            }
            let length_in_ray_dir = distance_in_z_direction / trans_dir.z;
            trans_pos_in_m += length_in_ray_dir * trans_dir;
        }
        Some((
            meter!(trans_pos_in_m.x, trans_pos_in_m.y, trans_pos_in_m.z),
            Vector3::new(0.0, 0.0, -1.0 * trans_dir.z.signum()),
        ))
    }
    fn isometry(&self) -> &Isometry {
        &self.isometry
    }
    fn set_isometry(&mut self, isometry: &Isometry) {
        self.isometry = isometry.clone();
    }
}
