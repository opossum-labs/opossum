use super::Surface;
use crate::ray::Ray;
use nalgebra::Point3;
use nalgebra::Vector3;

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
        None
    }
}
