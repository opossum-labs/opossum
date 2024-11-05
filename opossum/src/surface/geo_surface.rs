//! Module for handling geometric surfaces
//!
//! This module contains the [`GeoSurface`] trait which handles the interface for calculating things like intersection
//! points etc. and an enum containing the concrete surface types.

use super::{Cylinder, Parabola, Plane, Sphere};
use crate::{ray::Ray, utils::geom_transformation::Isometry};
use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};
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
    /// its normal vector of a [`Ray`].
    ///
    /// **Note**: Do not call this functions directly but rather
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

/// Enum for geometric surfaces, used in [`OpticSurface`]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum GeometricSurface {
    /// spherical surface. Holds a [`Sphere`] and a a flag that defines whether this surface is to be used as convex or concave
    Spherical {
        /// surface: [`Sphere`]
        s: Sphere,
    },
    /// flat surface that holds a [`Plane`]
    Flat {
        /// surface: [`Plane`]
        s: Plane,
    },
    /// parabolic surface that holds a [`Parabola`]
    Parabolic {
        /// surface: [`Parabola`]
        s: Parabola,
    },
    /// cylindrical surface that holds a [`Cylinder`]
    Cylindrical {
        /// surface: [`Cylinder`]
        s: Cylinder,
    },
}

impl GeoSurface for GeometricSurface {
    fn calc_intersect_and_normal_do(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        match self {
            Self::Spherical { s } => s.calc_intersect_and_normal_do(ray),
            Self::Flat { s } => s.calc_intersect_and_normal_do(ray),
            Self::Cylindrical { s } => s.calc_intersect_and_normal_do(ray),
            Self::Parabolic { s } => s.calc_intersect_and_normal_do(ray),
        }
    }

    fn isometry(&self) -> &Isometry {
        match self {
            Self::Spherical { s } => s.isometry(),
            Self::Flat { s } => s.isometry(),
            Self::Cylindrical { s } => s.isometry(),
            Self::Parabolic { s } => s.isometry(),
        }
    }

    fn set_isometry(&mut self, isometry: &Isometry) {
        match self {
            Self::Spherical { s } => s.set_isometry(isometry),
            Self::Flat { s } => s.set_isometry(isometry),
            Self::Cylindrical { s } => s.set_isometry(isometry),
            Self::Parabolic { s } => s.set_isometry(isometry),
        }
    }

    fn box_clone(&self) -> Box<dyn GeoSurface> {
        todo!()
    }
}

impl Default for GeometricSurface {
    fn default() -> Self {
        Self::Flat {
            s: Plane::new(&Isometry::identity()),
        }
    }
}

impl Debug for dyn GeoSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Surface")
    }
}
