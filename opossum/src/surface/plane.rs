//! Flat surface
//!
//! An infinitely large flat 2D surface oriented perpendicular to the optical axis (xy plane) and positioned a the given z position.

use super::Surface;
use crate::error::{OpmResult, OpossumError};
use crate::ray::Ray;
use nalgebra::{Point3, Vector3};
use uom::si::{f64::Length, length::meter};

#[derive(Debug)]
/// An infinitely large flat surface with its normal collinear to the optical axis.
pub struct Plane {
    z: Length,
}
impl Plane {
    /// Create a new [`Plane`] located at the given z position on the optical axis.
    ///
    /// The plane is oriented vertical with respect to the optical axis (xy plane).
    /// # Errors
    ///
    /// This function will return an error if z is not finite.
    pub fn new(z: Length) -> OpmResult<Self> {
        if !z.is_finite() {
            return Err(OpossumError::Other("z must be finite".into()));
        }
        Ok(Self { z })
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
        let intersection_point = Point3::new(
            Length::new::<meter>(ray_position.x),
            Length::new::<meter>(ray_position.y),
            Length::new::<meter>(z_in_meter),
        );
        let normal_vector = Vector3::new(0.0, 0.0, -1.0);
        Some((intersection_point, normal_vector))
    }
}

#[cfg(test)]
mod test {
    use uom::si::{
        energy::joule,
        f64::{Energy, Length},
        length::{millimeter, nanometer},
    };

    use super::*;
    #[test]
    fn new() {
        assert!(Plane::new(Length::new::<millimeter>(f64::NAN)).is_err());
        assert!(Plane::new(Length::new::<millimeter>(f64::NEG_INFINITY)).is_err());
        assert!(Plane::new(Length::new::<millimeter>(f64::INFINITY)).is_err());
        let p = Plane::new(Length::new::<millimeter>(1.0)).unwrap();
        assert_eq!(p.z, Length::new::<millimeter>(1.0));
    }
    #[test]
    fn intersect_on_axis() {
        let s = Plane::new(Length::new::<millimeter>(10.0)).unwrap();
        let ray = Ray::new(
            Point3::new(
                Length::new::<millimeter>(0.0),
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
            Some((
                Point3::new(
                    Length::new::<millimeter>(0.0),
                    Length::new::<millimeter>(0.0),
                    Length::new::<millimeter>(10.0)
                ),
                Vector3::new(0.0, 0.0, -1.0)
            ))
        );
    }
    #[test]
    fn intersect_on_axis_behind() {
        let s = Plane::new(Length::new::<millimeter>(-10.0)).unwrap();
        let ray = Ray::new(
            Point3::new(
                Length::new::<millimeter>(0.0),
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
        let s = Plane::new(Length::new::<millimeter>(10.0)).unwrap();
        let ray = Ray::new(
            Point3::new(
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0),
            ),
            Vector3::new(0.0, 0.0, 1.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((
                Point3::new(
                    Length::new::<millimeter>(0.0),
                    Length::new::<millimeter>(1.0),
                    Length::new::<millimeter>(10.0)
                ),
                Vector3::new(0.0, 0.0, -1.0)
            ))
        );
        let ray = Ray::new(
            Point3::new(
                Length::new::<millimeter>(0.0),
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(0.0),
            ),
            Vector3::new(0.0, 1.0, 1.0),
            Length::new::<nanometer>(1053.0),
            Energy::new::<joule>(1.0),
        )
        .unwrap();
        assert_eq!(
            s.calc_intersect_and_normal(&ray),
            Some((
                Point3::new(
                    Length::new::<millimeter>(0.0),
                    Length::new::<millimeter>(11.0),
                    Length::new::<millimeter>(10.0)
                ),
                Vector3::new(0.0, 0.0, -1.0)
            ))
        );
    }
}
