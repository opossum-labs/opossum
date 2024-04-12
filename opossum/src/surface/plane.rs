//! Flat surface
//!
//! An infinitely large flat 2D surface oriented perpendicular to the optical axis (xy plane) and positioned a the given z position.

use super::Surface;
use crate::ray::Ray;
use crate::render::{Color, Render, Renderable, SDF};
use crate::utils::geom_transformation::Isometry;
use crate::{
    error::{OpmResult, OpossumError},
    meter,
};
use approx::relative_eq;
use nalgebra::{Point3, Vector3};
use uom::si::f64::Length;

#[derive(Debug)]
/// An infinitely large flat surface with its normal collinear to the optical axis.
pub struct Plane {
    z: Length,
    normal: Vector3<f64>,
    anchor_point: Point3<Length>,
    shift: Length,
    isometry: Isometry,
}
impl Plane {
    /// Create a new [`Plane`] located at the given z position on the optical axis.
    ///
    /// The plane is oriented vertical with respect to the optical axis (xy plane).
    /// # Errors
    ///
    /// This function will return an error if z is not finite.
    pub fn new_along_z(z: Length) -> OpmResult<Self> {
        if !z.is_finite() {
            return Err(OpossumError::Other("z must be finite".into()));
        }
        let isometry = Isometry::new_from_view(Point3::origin(), Vector3::z(), Vector3::y());
        Ok(Self {
            z,
            normal: Vector3::z(),
            anchor_point: Point3::origin(),
            shift: z,
            isometry,
        })
    }
    /// Create a new [`Plane`] located at the given z position on the optical axis.
    ///
    /// The plane is oriented vertical with respect to the optical axis (xy plane).
    /// # Errors
    ///
    /// This function will return an error if z is not finite.
    pub fn new(z: Length, normal: Vector3<f64>, anchor_point: Point3<Length>) -> OpmResult<Self> {
        if !z.is_finite() {
            return Err(OpossumError::Other("z must be finite".into()));
        }
        if normal.iter().any(|x| !x.is_finite()) || relative_eq!(normal.norm(), 0.0) {
            return Err(OpossumError::Other(
                "normal vector components must be finite and its norm != 0!".into(),
            ));
        }
        if anchor_point.iter().any(|x| !x.is_finite()) {
            return Err(OpossumError::Other(
                "anchor_point components must be finite!".into(),
            ));
        }
        let isometry = Isometry::new(anchor_point, Vector3::new(0., 0., 0.));
        let shift = (anchor_point.x * anchor_point.x
            + anchor_point.y * anchor_point.y
            + anchor_point.z * anchor_point.z)
            .sqrt();
        Ok(Self {
            z,
            normal: normal.normalize(),
            anchor_point,
            shift,
            isometry,
        })
    }

    /// Returns the anchor point of this plane
    #[must_use]
    pub const fn get_anchor_point(&self) -> Point3<Length> {
        self.anchor_point
    }
}

impl Surface for Plane {
    fn calc_intersect_and_normal(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        let mut ray_position = ray.position().map(|c| c.value);
        let ray_direction = ray.direction();
        let z_in_meter = self.z.value;
        let distance_in_z_direction = z_in_meter - ray_position.z;
        if distance_in_z_direction.signum() != ray_direction.z.signum() {
            // Ray propagates away from the plane => no intersection
            return None;
        }
        let length_in_ray_dir = distance_in_z_direction / ray_direction.z;
        ray_position += length_in_ray_dir * ray_direction;
        let intersection_point = meter!(ray_position.x, ray_position.y, self.z.value);
        let normal_vector = Vector3::new(0.0, 0.0, -1.0);
        Some((intersection_point, normal_vector))
    }
}
impl Color for Plane {
    fn get_color(&self, _p: &Point3<f64>) -> Vector3<f64> {
        Vector3::new(0.3, 0.3, 0.3)
    }
}
impl SDF for Plane {
    fn sdf_eval_point(&self, p: &Point3<f64>) -> f64 {
        let p_out = self.isometry.inverse_transform_point_f64(p);
        p_out.x.mul_add(self.normal.x, p_out.y * self.normal.y)
            + p_out.z.mul_add(self.normal.z, self.shift.value)
    }
}
impl Render<'_> for Plane {}
impl Renderable<'_> for Plane {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{joule, millimeter, nanometer};

    #[test]
    fn new() {
        assert!(Plane::new_along_z(millimeter!(f64::NAN)).is_err());
        assert!(Plane::new_along_z(millimeter!(f64::NEG_INFINITY)).is_err());
        assert!(Plane::new_along_z(millimeter!(f64::INFINITY)).is_err());
        let p = Plane::new_along_z(millimeter!(1.0)).unwrap();
        assert_eq!(p.z, millimeter!(1.0));
    }
    #[test]
    fn intersect_on_axis() {
        let s = Plane::new_along_z(millimeter!(10.0)).unwrap();
        let ray = Ray::new(
            millimeter!(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((millimeter!(0.0, 0.0, 10.0), Vector3::new(0.0, 0.0, -1.0)))
        );
    }
    #[test]
    fn intersect_on_axis_behind() {
        let s = Plane::new_along_z(millimeter!(-10.0)).unwrap();
        let ray = Ray::new(
            millimeter!(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        assert_eq!(s.calc_intersect_and_normal(&ray), None);
    }
    #[test]
    fn intersect_off_axis() {
        let s = Plane::new_along_z(millimeter!(10.0)).unwrap();
        let ray = Ray::new(
            millimeter!(0.0, 1.0, 1.0),
            Vector3::new(0.0, 0.0, 1.0),
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((millimeter!(0.0, 1.0, 10.0), Vector3::new(0.0, 0.0, -1.0)))
        );
        let ray = Ray::new(
            millimeter!(0.0, 1.0, 0.0),
            Vector3::new(0.0, 1.0, 1.0),
            nanometer!(1053.0),
            joule!(1.0),
        )
        .unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((millimeter!(0.0, 11.0, 10.0), Vector3::new(0.0, 0.0, -1.0)))
        );
    }
}
