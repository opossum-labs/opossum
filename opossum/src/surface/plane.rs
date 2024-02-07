//! An infinitely large flat 2D surface oriented perpendicular to the optical axis.

use super::Surface;
use crate::ray::Ray;
use nalgebra::Point3;
use nalgebra::Vector3;
use uom::si::length::millimeter;

pub struct Plane {
    z: f64,
}
impl Plane {
    pub fn new(z: f64) -> Self {
        Plane { z }
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
