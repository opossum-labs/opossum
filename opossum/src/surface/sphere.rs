use super::Surface;
use crate::ray::Ray;
use nalgebra::Point3;
use nalgebra::Vector3;
use roots::find_roots_quadratic;
use roots::Roots;
use uom::si::length::millimeter;

pub struct Sphere {
    z: f64,
    radius: f64,
}
impl Sphere {
    pub fn new(z: f64, radius: f64) -> Self {
        Sphere { z, radius }
    }
}
impl Surface for Sphere {
    fn calc_intersect_and_normal(&self, ray: &Ray) -> Option<(Point3<f64>, Vector3<f64>)> {
        // sphere formula
        // x^2 + y^2 + (z-z_0)^2 = r^2
        //
        // insert ray (p: position, d: direction):
        // (p_x+t*d_x)^2 + (p_y+t*d_y)^2 + (p_z+t*d_z-z_0)^2 - r^2 = 0
        // This translates into th qudratic equation
        // at^2 + bt + c = 0 with
        // a = d_x^2 + d_y^2 + d_z^2
        // b = 2 (d_x * p_x + d_y * p_y + d_z *(p_z - z_0))
        // c = p_x^2 + p_y^2 + p_z^2 - 2*z_0*p_z - z_0^2 - r^2
        let d = ray.direction();
        let p = ray.position().map(|c| c.get::<millimeter>());
        let a = d.x * d.x + d.y * d.y + d.z * d.z;
        let b = 2.0 * (d.x * p.x + d.y * p.y + d.z * (p.z - self.z));
        let c = p.x * p.x + p.y * p.y + p.z * p.z
            - 2.0 * self.z * p.z
            - self.z * self.z
            - self.radius * self.radius;
        // Solve t of qudaratic equation
        
        let intersection_point = Point3::new(0.0, 0.0, 0.0);
        let center_point = Point3::new(0.0, 0.0, self.z);
        let _normal_vector = intersection_point - center_point;
        None
    }
}
