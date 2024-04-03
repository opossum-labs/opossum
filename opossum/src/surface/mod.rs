#![warn(missing_docs)]
//! Module for handling optical surfaces

pub use cylinder::Cylinder;
pub use cuboid::Cuboid;
mod plane;
mod sphere;
mod cuboid;
mod cylinder;
pub use plane::Plane;
pub use sphere::Sphere;

use crate::ray::Ray;
use nalgebra::{Point3, Vector3};
use std::fmt::Debug;
use uom::si::f64::Length;

/// Trait for handling optical surfaces.
///
/// An optical surface such as [`Plane`] or [`Sphere`] has to implement this trait in order to be used by the
/// `ray.refract_on_surface` function.
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
