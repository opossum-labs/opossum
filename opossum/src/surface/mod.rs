#![warn(missing_docs)]
//! Module for handling geometric surfaces.
//!
//! This module handles only the geometric aspect of an optical surface. So a [`GeoSurface`] has no [`Coating`](crate::coatings::Coating) or
//! [`Aperture`](crate::aperture::Aperture)s.
mod cylinder;
mod plane;
mod sphere;

pub mod hit_map;
mod optical_surface;
pub use cylinder::Cylinder;
pub use optical_surface::OpticalSurface;
pub use plane::Plane;
pub use sphere::Sphere;

use crate::{ray::Ray, utils::geom_transformation::Isometry};
use nalgebra::{Point3, Vector3};
use std::fmt::Debug;
use uom::si::f64::Length;

/// Trait for handling geometric surfaces.
///
/// A geomatric surface such as [`Plane`] or [`Sphere`] has to implement this trait in order to be used by the
/// `ray.refract_on_surface` function.
pub trait GeoSurface {
    /// Calculate intersection point and its normal vector of a [`Ray`] with a [`GeoSurface`]
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
    /// This fucntion must be implemented by all [`GeoSurface`]s for calculating the intersection point and
    /// its normal vector of a [`Ray`]. **Note**: Do not call this functions directly but rather
    /// `calc_intersect_and_normal` which is a wrapper handling all isometric transformations. The implemented function
    /// does not need to consider any isometries.
    ///
    /// This function returns `None` if the given ray does not intersect with the surface.
    fn calc_intersect_and_normal_do(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)>;
    /// Returns the [`Isometry`] of this [`GeoSurface`].
    fn isometry(&self) -> &Isometry;
    /// Set the [`Isometry`] of this [`GeoSurface`].
    ///
    /// This function can be used to place and align the [`GeoSurface`] in 3D space.
    fn set_isometry(&mut self, isometry: &Isometry);
    /// Create a clone of this [`GeoSurface`].
    ///
    /// **Note**: This has to be done explicitly since there is no `clone` for a trait.
    fn box_clone(&self) -> Box<dyn GeoSurface>;
}

impl Debug for dyn GeoSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Surface")
    }
}
