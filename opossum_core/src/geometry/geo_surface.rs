//! Module for handling geometric surfaces
//!
//! This module contains the [`GeoSurface`] trait which handles the interface for calculating things like intersection
//! points etc. and an enum containing the concrete surface types.

use super::Plane;
use crate::{light::Ray, utils::geom_transformation::Isometry};
use nalgebra::{Point2, Point3, Vector3};
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
    /// "Behind" refers to the side a ray travelling along the local z axis reaches *after* passing
    /// the surface. This is the test a bounded body is built from: a point inside a body lies
    /// behind the body's entrance surface but not behind its exit surface (see
    /// [`Body`](super::body::Body)).
    ///
    /// **Note**: This is a strict half space only for the surfaces that are one, i.e.
    /// [`Plane`] and [`Parabola`](super::Parabola). [`Sphere`](super::Sphere) and
    /// [`Cylinder`](super::Cylinder) answer "inside the ball / cylinder of their radius", which
    /// coincides with the half space near the vertex and departs from it towards the rim, where
    /// the surface curves away and eventually closes on itself. Composing two of them into a body
    /// is therefore exact as long as the body reaches less far transversally than its radii of
    /// curvature — which is what a clear aperture normally guarantees.
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
    /// Return the longitudinal position of this [`GeoSurface`] above a given transversal position,
    /// in the surface's own local frame.
    ///
    /// Every surface modelled here is a profile over its local xy plane: exactly one point of it
    /// lies above each transversal position it reaches. This states where that point is, which is
    /// what a bounded region built from such surfaces needs in order to know how far it extends
    /// along the optical axis — see [`Body::bounding_box`](super::body::Body::bounding_box).
    ///
    /// The origin yields the surface's **anchor point**, i.e. its vertex: the point its placement
    /// refers to, and the point the sag of a curved surface is measured from. Note that this is not
    /// generally the origin of the local frame — [`Sphere`](super::Sphere) and
    /// [`Cylinder`](super::Cylinder) are centered on their center of curvature, so their vertex sits
    /// one radius away from it.
    ///
    /// # Arguments
    ///
    /// - `transversal_position`: the position in the local xy plane to look above, given in the
    ///   local frame of this surface.
    ///
    /// # Returns
    ///
    /// The local z coordinate of the surface above the given position, or `None` if the surface does
    /// not reach that far out. Only the curved surfaces of finite extent can answer `None`: a
    /// [`Sphere`](super::Sphere) or [`Cylinder`](super::Cylinder) ends where its radius of curvature
    /// does.
    fn local_z_at(&self, transversal_position: &Point2<Length>) -> Option<Length>;
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

/// Determine the local z position of a curved surface above a given transversal distance from its
/// axis.
///
/// Shared by the surfaces whose local frame is centered on their center of curvature
/// ([`Sphere`](super::Sphere), [`Cylinder`](super::Cylinder)) — the same pair, and for the same
/// reason, as [`is_behind_curvature`]: they differ only in how that distance is measured. The
/// vertex, at distance zero, therefore lies at `-radius` for either sign of the curvature.
///
/// # Arguments
///
/// - `distance_from_axis`: transversal distance of the position from the surface's axis, in meter
/// - `radius`: the signed radius of curvature, in meter
///
/// # Returns
///
/// The local z coordinate of the surface, in meter, or `None` beyond the radius of curvature: there
/// the surface has already curved back on itself and no longer lies above the transversal plane.
pub(super) fn curved_local_z(distance_from_axis: f64, radius: f64) -> Option<f64> {
    let half_chord_squared = radius.mul_add(radius, -(distance_from_axis * distance_from_axis));
    (half_chord_squared >= 0.0).then(|| -radius.signum() * half_chord_squared.sqrt())
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
