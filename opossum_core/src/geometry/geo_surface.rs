//! Module for handling geometric surfaces
//!
//! This module contains the [`GeoSurface`] trait which handles the interface for calculating things like intersection
//! points etc. and an enum containing the concrete surface types.

use super::Plane;
use crate::{light::Ray, utils::geom_transformation::Isometry};
use nalgebra::{Point3, Vector3};
use std::{
    fmt::Debug,
    sync::{Arc, Mutex},
};
use uom::si::f64::Length;

/// Trait for handling geometric surfaces.
///
/// A geometric surface such as [`Plane`] or [`Sphere`](super::sphere::Sphere) has to implement this trait in order to be used by the
/// `ray.refract_on_surface` function.
pub trait GeoSurface: Send + Sync + Debug {
    /// Calculate intersection point and its normal vector of a [`Ray`] with a [`GeoSurface`]
    ///
    /// The surface normal is guaranteed to point always against the ray direction and is normalized.
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
    /// **Note2**: It is assumed that the surface normal always points against the ray direction and is normalized.
    ///
    /// This function returns `None` if the given ray does not intersect with the surface.
    fn calc_intersect_and_normal_do(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)>;
    /// Return whether the given point lies behind this [`GeoSurface`].
    ///
    /// "Behind" refers to the half space on the positive z side of the surface in its own local
    /// frame — the side a ray travelling along the local z axis reaches *after* passing the
    /// surface. This is the half-space test a bounded body is built from: a point inside a body
    /// lies behind the body's entrance surface but not behind its exit surface (see
    /// [`Body`](super::body::Body)).
    ///
    /// # Arguments
    ///
    /// - `point`: the point to be tested, given in global coordinates.
    ///
    /// # Returns
    ///
    /// `true` if the point lies behind the surface. Points exactly on the surface count as behind.
    fn is_behind(&self, point: &Point3<Length>) -> bool {
        let local_point = self.isometry().inverse_transform_point(point);
        self.is_behind_do(&local_point)
    }
    /// This function must be implemented by all [`GeoSurface`]s for deciding on which side of the
    /// surface a given point lies.
    ///
    /// **Note**: Do not call this function directly but rather `is_behind` which is a wrapper
    /// handling all isometric transformations. The implemented function does not need to consider
    /// any isometries.
    ///
    /// # Arguments
    ///
    /// - `point`: the point to be tested, already transformed into the local frame of the surface.
    ///
    /// # Returns
    ///
    /// `true` if the point lies behind the surface. Points exactly on the surface count as behind.
    fn is_behind_do(&self, point: &Point3<Length>) -> bool;
    /// Returns the [`Isometry`] of this [`GeoSurface`].
    fn isometry(&self) -> &Isometry;
    /// Set the [`Isometry`] of this [`GeoSurface`].
    ///
    /// This function can be used to place and align the [`GeoSurface`] in 3D space.
    fn set_isometry(&mut self, isometry: Isometry);
    /// Return the surface type as string (for debugging purposes)
    fn name(&self) -> &str;
}

/// Decide whether a point lies behind a curved surface, given its distance from the center of
/// curvature.
///
/// Shared by the surfaces whose local frame is centered on their center of curvature
/// ([`Sphere`](super::Sphere), [`Cylinder`](super::Cylinder)), which differ only in how that
/// distance is measured. For a convex surface (positive radius) the center of curvature lies behind
/// the surface, for a concave one (negative radius) in front of it.
///
/// # Arguments
///
/// - `distance_from_center`: distance of the point from the center of curvature, in meter
/// - `radius`: the signed radius of curvature, in meter
///
/// # Returns
///
/// `true` if the point lies behind the surface. Points exactly on the surface count as behind.
pub(super) const fn is_behind_curvature(distance_from_center: f64, radius: f64) -> bool {
    if radius.is_sign_positive() {
        distance_from_center <= radius
    } else {
        distance_from_center >= -radius
    }
}

/// Reference for a [`GeoSurface`].
///
/// This struct is necessary in order to implement a Default trait on a `Arc<Mutex<GeoSurface>>`.
#[derive(Clone, Debug)]
pub struct GeoSurfaceRef(pub Arc<Mutex<dyn GeoSurface>>);

impl Default for GeoSurfaceRef {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(Plane::default())))
    }
}

#[cfg(test)]
mod test_geo_surface_ref {}
