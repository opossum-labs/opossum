//! An infinitely large flat 2D surface oriented perpendicular to the optical axis.

use super::Surface;
use crate::error::OpmResult;
use crate::error::OpossumError;
use crate::ray::Ray;
use nalgebra::Point3;
use nalgebra::Vector3;
use uom::si::length::millimeter;

pub struct Plane {
    z: f64,
}
impl Plane {
    /// Create a new [`Plane`] located at the given z position on the optical axis.
    ///
    /// # Errors
    ///
    /// This function will return an error if z is not finite.
    pub fn new(z: f64) -> OpmResult<Self> {
        if !z.is_finite() {
            return Err(OpossumError::Other("z must be finite".into()));
        }
        Ok(Self { z })
    }
}

impl Surface for Plane {
    fn calc_intersect_and_normal(&self, ray: &Ray) -> Option<(Point3<f64>, Vector3<f64>)> {
        let mut ray_position = ray.position().map(|c| c.get::<millimeter>());
        let ray_direction = ray.direction();
        let distance_in_z_direction = self.z - ray_position.z;
        if distance_in_z_direction.signum() != ray_direction.z.signum() {
            // Ray propagates away from the plane => no intersection
            return None;
        }
        let length_in_ray_dir = distance_in_z_direction / ray_direction.z;
        ray_position += length_in_ray_dir * ray_direction;
        let intersection_point = Point3::new(ray_position.x, ray_position.y, self.z);
        let normal_vector = Vector3::new(0.0, 0.0, -1.0);
        Some((intersection_point, normal_vector))
    }
}

#[cfg(test)]
mod test {
    use nalgebra::Point2;
    use uom::si::{
        energy::joule,
        f64::{Energy, Length},
        length::nanometer,
    };

    use super::*;
    #[test]
    fn new() {
        assert!(Plane::new(f64::NAN).is_err());
        assert!(Plane::new(f64::NEG_INFINITY).is_err());
        assert!(Plane::new(f64::INFINITY).is_err());
        let p = Plane::new(1.0).unwrap();
        assert_eq!(p.z, 1.0);
    }
    #[test]
    fn intersect_on_axis() {
        let s = Plane::new(10.0).unwrap();
        let ray = Ray::new(
            Point2::new(
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(0.0),
            ),
            Vector3::new(0.0, 0.0, 1.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((Point3::new(0.0, 0.0, 10.0), Vector3::new(0.0, 0.0, -1.0)))
        );
    }
    #[test]
    fn intersect_on_axis_behind() {
        let s = Plane::new(-10.0).unwrap();
        let ray = Ray::new(
            Point2::new(
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(0.0),
            ),
            Vector3::new(0.0, 0.0, 1.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        assert_eq!(s.calc_intersect_and_normal(&ray), None);
    }
    #[test]
    fn intersect_off_axis() {
        let s = Plane::new(10.0).unwrap();
        let ray = Ray::new(
            Point2::new(
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(1.0),
            ),
            Vector3::new(0.0, 0.0, 1.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((Point3::new(0.0, 1.0, 10.0), Vector3::new(0.0, 0.0, -1.0)))
        );
        let ray = Ray::new(
            Point2::new(
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(1.0),
            ),
            Vector3::new(0.0, 1.0, 1.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((Point3::new(0.0, 11.0, 10.0), Vector3::new(0.0, 0.0, -1.0)))
        );
    }
}
