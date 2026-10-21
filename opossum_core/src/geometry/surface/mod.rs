#![warn(missing_docs)]
//! The kinds of unbounded surface a face can have, and the math of each.
//!
//! A [`SurfaceShape`] names the kind of a surface and holds its parameters. Each kind lives in a
//! file of its own and answers the questions of [`GeoSurface`] in its own vertex frame: the vertex
//! at the origin, the axis along z. Where a surface sits in the world is the business of the
//! [`Face`](crate::geometry::Face) it belongs to, which carries the frame.

mod cylinder;
mod parabola;
mod plane;
mod sphere;

pub use cylinder::Cylinder;
pub use parabola::Parabola;
pub use plane::Plane;
pub use sphere::Sphere;

use nalgebra::{Point2, Point3, Vector3};
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use crate::{error::OpmResult, light::Ray};

/// The shape of an optical surface, independent of where it is placed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SurfaceShape {
    /// A flat surface.
    Plane,
    /// A spherical surface.
    Sphere(Sphere),
    /// A cylindrical surface, curved along the local x axis.
    Cylinder(Cylinder),
    /// A parabolic surface.
    Parabola(Parabola),
}

impl SurfaceShape {
    /// A spherical surface with the given radius of curvature.
    ///
    /// # Arguments
    ///
    /// * `radius` - the radius of curvature.
    ///
    /// # Returns
    ///
    /// [`SurfaceShape::Sphere`], or [`SurfaceShape::Plane`] for an infinite radius.
    ///
    /// # Errors
    ///
    /// This function returns an error if the radius is zero or not a number.
    pub fn spherical(radius: Length) -> OpmResult<Self> {
        if radius.value.is_infinite() {
            return Ok(Self::Plane);
        }
        Ok(Self::Sphere(Sphere::new(radius)?))
    }
    /// A cylindrical surface with the given radius of curvature.
    ///
    /// # Arguments
    ///
    /// * `radius` - the radius of curvature.
    ///
    /// # Returns
    ///
    /// [`SurfaceShape::Cylinder`], or [`SurfaceShape::Plane`] for an infinite radius.
    ///
    /// # Errors
    ///
    /// This function returns an error if the radius is zero or not a number.
    pub fn cylindrical(radius: Length) -> OpmResult<Self> {
        if radius.value.is_infinite() {
            return Ok(Self::Plane);
        }
        Ok(Self::Cylinder(Cylinder::new(radius)?))
    }
    /// Return the surface of this shape, to be asked in its own vertex frame.
    #[must_use]
    pub fn as_geo_surface(&self) -> &dyn GeoSurface {
        match self {
            Self::Plane => &Plane,
            Self::Sphere(sphere) => sphere,
            Self::Cylinder(cylinder) => cylinder,
            Self::Parabola(parabola) => parabola,
        }
    }
    /// Whether this shape is the same as another one, up to a tolerance on its radius or focal length.
    ///
    /// # Arguments
    ///
    /// * `other` - the shape to compare with.
    /// * `tolerance` - the largest difference of radius or focal length still counted as equal.
    ///
    /// # Returns
    ///
    /// `true` if both are the same kind of surface and their radius or focal length agree within
    /// `tolerance`.
    pub(crate) fn matches(&self, other: &Self, tolerance: Length) -> bool {
        let close = |a: Length, b: Length| (a - b).abs() <= tolerance;
        match (self, other) {
            (Self::Plane, Self::Plane) => true,
            (Self::Sphere(one), Self::Sphere(other)) => close(one.radius(), other.radius()),
            (Self::Cylinder(one), Self::Cylinder(other)) => close(one.radius(), other.radius()),
            (Self::Parabola(one), Self::Parabola(other)) => {
                close(one.focal_length(), other.focal_length())
            }
            _ => false,
        }
    }
    /// Whether turning this shape about its own axis (local z) changes it.
    ///
    /// # Returns
    ///
    /// `false` for a shape that is rotationally symmetric about its axis. `true` for a
    /// [`SurfaceShape::Cylinder`], which is curved along its local x axis only; half a turn still
    /// maps it onto itself.
    pub(crate) const fn depends_on_roll(&self) -> bool {
        match self {
            Self::Plane | Self::Sphere(_) | Self::Parabola(_) => false,
            Self::Cylinder(_) => true,
        }
    }
}

/// The math of an unbounded surface, in its own vertex frame.
///
/// The vertex lies at the origin and the surface faces along z; a ray travelling along +z meets it
/// from the front. Placing the surface somewhere is left to the [`Face`](crate::geometry::Face) it
/// belongs to, which carries the frame and turns rays and points into this one.
pub trait GeoSurface {
    /// Calculate the intersection point of a [`Ray`] with this surface and the normal there.
    ///
    /// # Arguments
    ///
    /// * `ray` - the ray, in the vertex frame of this surface.
    ///
    /// # Returns
    ///
    /// The intersection point and the normalized surface normal there, pointing against the ray
    /// direction, both in the vertex frame; or `None` if the ray does not meet the surface.
    fn calc_intersect_and_normal(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)>;
    /// Return whether a point lies behind this surface.
    ///
    /// "Behind" refers to the side a ray travelling along the local z axis reaches *after* passing
    /// the surface. This is the test a bounded body is built from: a point inside a body lies
    /// behind the body's entrance surface but not behind its exit surface (see
    /// [`Body`](crate::geometry::body::Body)).
    ///
    /// **Note**: This is a strict half space only for the surfaces that are one, i.e. [`Plane`]
    /// and [`Parabola`]. [`Sphere`] and [`Cylinder`] answer "inside the ball / cylinder of their
    /// radius", which coincides with the half space near the vertex and departs from it towards
    /// the rim, where the surface curves away and eventually closes on itself. Composing two of
    /// them into a body is therefore exact as long as the body reaches less far transversally than
    /// its radii of curvature — which is what an optic shape normally guarantees.
    ///
    /// # Arguments
    ///
    /// - `point`: the point to be tested, in the vertex frame of this surface.
    ///
    /// # Returns
    ///
    /// `true` if the point lies behind the surface. Points exactly on the surface count as behind.
    fn is_behind(&self, point: &Point3<Length>) -> bool;
    /// Return the longitudinal position of this surface above a given transversal position.
    ///
    /// Every surface modelled here is a profile over its local xy plane: exactly one point of it
    /// lies above each transversal position it reaches on the sheet through its vertex. This states
    /// where that point is, which is what a bounded region built from such surfaces needs in order
    /// to know how far it extends along the optical axis — see
    /// [`Body::bounding_box`](crate::geometry::body::Body::bounding_box). The origin yields the
    /// vertex, at z = 0.
    ///
    /// # Arguments
    ///
    /// - `transversal_position`: the position in the local xy plane to look above.
    ///
    /// # Returns
    ///
    /// The local z coordinate of the surface above the given position, or `None` if the surface does
    /// not reach that far out. Only the curved surfaces of finite extent can answer `None`: a
    /// [`Sphere`] or [`Cylinder`] ends where its radius of curvature does.
    fn local_z_at(&self, transversal_position: &Point2<Length>) -> Option<Length>;
    /// Return whether this surface reaches the given distance from its axis.
    ///
    /// A sphere or a cylinder ends at its radius of curvature; exactly reaching it (a hemisphere)
    /// is still a shape. A cylinder is held to its radius in every direction, as a body bounded by
    /// it is. The other surfaces reach arbitrarily far.
    ///
    /// # Arguments
    ///
    /// * `distance` - the distance from the axis to be reached.
    fn reaches(&self, distance: Length) -> bool;
    /// Return the kind of this surface as a name, for messages.
    fn name(&self) -> &'static str;
}

/// Decide whether a point lies behind a curved surface, given its distance from the center of
/// curvature.
///
/// Shared by the surfaces curved about a center of curvature ([`Sphere`], [`Cylinder`]), which
/// differ only in how that distance is measured. For a convex surface (positive radius) the center
/// of curvature lies behind the surface, for a concave one (negative radius) in front of it.
///
/// # Arguments
///
/// - `distance_from_center`: distance of the point from the center of curvature, in meter
/// - `radius`: the signed radius of curvature, in meter
///
/// # Returns
///
/// `true` if the point lies behind the surface. Points exactly on the surface count as behind.
const fn is_behind_curvature(distance_from_center: f64, radius: f64) -> bool {
    if radius.is_sign_positive() {
        distance_from_center <= radius
    } else {
        distance_from_center >= -radius
    }
}

/// Determine the local z position of a curved surface above a given transversal distance from its
/// axis, in its vertex frame.
///
/// Shared by [`Sphere`] and [`Cylinder`], for the same reason as [`is_behind_curvature`]: they
/// differ only in how that distance is measured. The center of curvature lies one radius behind
/// the vertex, so the vertex, at distance zero, lies at z = 0 for either sign of the curvature.
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
fn curved_local_z(distance_from_axis: f64, radius: f64) -> Option<f64> {
    let half_chord_squared = radius.mul_add(radius, -(distance_from_axis * distance_from_axis));
    (half_chord_squared >= 0.0)
        .then(|| (-radius.signum()).mul_add(half_chord_squared.sqrt(), radius))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::millimeter;

    #[test]
    fn an_infinite_radius_is_flat() -> OpmResult<()> {
        assert_eq!(
            SurfaceShape::spherical(millimeter!(f64::INFINITY))?,
            SurfaceShape::Plane
        );
        assert_eq!(
            SurfaceShape::cylindrical(millimeter!(f64::NEG_INFINITY))?,
            SurfaceShape::Plane
        );
        assert_eq!(
            SurfaceShape::spherical(millimeter!(100.0))?,
            SurfaceShape::Sphere(Sphere::new(millimeter!(100.0))?)
        );
        assert!(SurfaceShape::spherical(millimeter!(0.0)).is_err());
        Ok(())
    }
}
