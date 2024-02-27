//! Module for handling optical surfaces

mod plane;
mod sphere;

use std::fmt::Debug;

pub use plane::Plane;
pub use sphere::Sphere;
use uom::si::f64::Length;

use crate::ray::Ray;
use nalgebra::{Point3, Vector3};

pub trait Surface {
    /// Calculate intersection point and its normal vector of a [`Ray`] with a [`Surface`]
    ///
    /// This function returns `None` if the given ray does not intersect with the surface.
    fn calc_intersect_and_normal(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)>;
}

impl Debug for dyn Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Surface")
    }
}
