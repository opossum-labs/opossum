#![warn(missing_docs)]
//! Module for handling optical surfaces

//mod cuboid;
mod cylinder;
//mod optical_table;
mod plane;
mod sphere;
//pub use cuboid::Cuboid;
pub use cylinder::Cylinder;
//pub use optical_table::OpticalTable;
pub use plane::Plane;
pub use sphere::Sphere;

use crate::{ray::Ray, utils::geom_transformation::Isometry};
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
    fn calc_intersect_and_normal(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        let transformed_ray = ray.inverse_transformed_ray(self.isometry());
        if let Some((refracted, normal)) = self.calc_intersect_and_normal_do(&transformed_ray) {
            Some((
                self.isometry().transform_point(&refracted),
                self.isometry().transform_vector_f64(&normal),
            ))
        } else {
            None
        }
    }
    /// This fucntion must be implemented by all [`Surface`]s for calculating the intersection point and
    /// its normal vector of a [`Ray`]. **Note**: Do not calls this functions directly but rather
    /// `calc_intersect_and_normal` which is a wrapper handling all isometric transformations. The implemented function
    /// does not need to consider any isometries.
    ///
    /// This function returns `None` if the given ray does not intersect with the surface.
    fn calc_intersect_and_normal_do(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)>;
    /// Returns the [`Isometry`] of this [`Surface`].
    fn isometry(&self) -> &Isometry;
    /// Set the [`Isometry`] of this [`Surface`].
    ///
    /// This function can be used to place and align the [`Surface`] in 3D space.
    fn set_isometry(&mut self, isometry: &Isometry);
}

impl Debug for dyn Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Surface")
    }
}
