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

use nalgebra::{Point2, Point3, Vector2, Vector3};
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
    /// Return the normal of this surface above a given transversal position, in its vertex frame.
    ///
    /// The counterpart to [`local_z_at`](GeoSurface::local_z_at): that one states *where* the
    /// surface lies above a transversal position, this one states *which way it faces* there. Both
    /// are read off the shape of the surface itself rather than from a ray meeting it —
    /// [`calc_intersect_and_normal`](GeoSurface::calc_intersect_and_normal) only promises a normal
    /// pointing against the ray, which is a property of that ray and not of the surface.
    ///
    /// The normal is normalized and never points away from local +z, so it is the outward normal of
    /// a body this surface bounds from below and the inward one of a body it bounds from above. A
    /// curved surface tilts its normal away from +z towards the rim, and exactly at the rim, where
    /// the surface has turned by a right angle, the normal is purely transversal.
    ///
    /// # Arguments
    ///
    /// - `transversal_position`: the position in the local xy plane to look above.
    ///
    /// # Returns
    ///
    /// The unit normal above the given position, or `None` wherever
    /// [`local_z_at`](GeoSurface::local_z_at) finds no surface either.
    fn local_normal_at(&self, transversal_position: &Point2<Length>) -> Option<Vector3<f64>>;
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

/// How far `half_chord_squared` may dip below zero in [`curved_local_z`] before a position genuinely
/// counts as beyond the surface's curvature, rather than merely landing there because of rounding.
///
/// A hemisphere — curvature radius equal to the aperture radius — is the tightest a spherical or
/// cylindrical surface can be without folding back on itself, and geometrically valid: at the rim,
/// `distance_from_axis` equals `radius` exactly. But `distance_from_axis` is usually not `radius`
/// bit-for-bit there: it comes from sampling the outline (points on a circle of that radius,
/// reconstructed through trigonometric functions), whose own rounding can place a rim point a few
/// floating point epsilons beyond it. Squaring that in `half_chord_squared` then yields a result
/// that is negative although the true value is zero, and the surface would reject the very rim of
/// a valid hemisphere as unreachable.
///
/// Relative to `radius²` rather than an absolute distance, so the same tolerance holds whether the
/// surface is measured in micrometres or metres. The value sits far above the rounding this exists
/// for (a few times [`f64::EPSILON`]) and far below any curvature-versus-aperture mismatch a real
/// component would be built with.
const CURVATURE_RIM_TOLERANCE: f64 = 1e-9;

/// Determine the half chord of a curved surface above a given transversal distance from its axis:
/// how far the surface lies from its center of curvature along the axis there.
///
/// A distance up to [`CURVATURE_RIM_TOLERANCE`] beyond `radius` is treated as exactly `radius` —
/// the rim of an exact hemisphere, not a surface asked about a point it cannot reach.
///
/// # Returns
///
/// The half chord, in meter, or `None` beyond the radius of curvature.
fn half_chord(distance_from_axis: f64, radius: f64) -> Option<f64> {
    let half_chord_squared = radius.mul_add(radius, -(distance_from_axis * distance_from_axis));
    if half_chord_squared >= 0.0 {
        Some(half_chord_squared.sqrt())
    } else {
        (half_chord_squared >= -CURVATURE_RIM_TOLERANCE * radius * radius).then_some(0.0)
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
    half_chord(distance_from_axis, radius).map(|chord| (-radius.signum()).mul_add(chord, radius))
}

/// Determine the local normal of a curved surface above a given transversal position, in its
/// vertex frame.
///
/// Shared by [`Sphere`] and [`Cylinder`], for the same reason as [`curved_local_z`]: they differ
/// only in which transversal directions bend the surface. The normal runs along the line from the
/// center of curvature to the surface point, which dividing by the *signed* radius turns towards
/// local +z for either sign of the curvature.
///
/// # Arguments
///
/// - `curving`: the transversal position in the directions the surface curves in, in meter — both
///   components for a [`Sphere`], the x offset alone for a [`Cylinder`]
/// - `radius`: the signed radius of curvature, in meter
///
/// # Returns
///
/// The unit normal pointing towards local +z, or `None` beyond the radius of curvature, where
/// [`curved_local_z`] finds no surface either.
fn curved_local_normal(curving: Vector2<f64>, radius: f64) -> Option<Vector3<f64>> {
    let chord = half_chord(curving.norm(), radius)?;
    Some(Vector3::new(
        -curving.x / radius,
        -curving.y / radius,
        chord / radius.abs(),
    ))
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

#[cfg(test)]
mod test_curved_local_z {
    use super::*;

    /// Exactly at the radius, a curved surface has curved all the way round to its rim: one radius
    /// along the axis from its vertex, the rim of a hemisphere.
    #[test]
    fn exactly_at_the_radius_is_the_rim() {
        for radius in [0.025_f64, -0.025] {
            assert_eq!(
                curved_local_z(radius.abs(), radius),
                Some(radius),
                "radius {radius}"
            );
        }
    }

    /// A distance a few floating point epsilons beyond the radius still counts as the rim - this is
    /// the whole reason [`CURVATURE_RIM_TOLERANCE`] exists: the outline of a hemisphere is reached
    /// through a trigonometric sampling while the curvature radius is used as given, so the two
    /// rarely agree bit-for-bit even where they are mathematically equal.
    #[test]
    fn a_touch_beyond_the_radius_within_tolerance_still_reaches() {
        let radius = 0.025;
        // A tenth of the tolerance, so the test sits comfortably inside the allowed band.
        let just_past = radius * (1.0 + CURVATURE_RIM_TOLERANCE / 10.0);
        assert_eq!(curved_local_z(just_past, radius), Some(radius));
        assert_eq!(curved_local_z(just_past, -radius), Some(-radius));
    }

    /// A distance genuinely beyond the radius still does not reach: the tolerance is only a narrow
    /// allowance for where the surface already is.
    #[test]
    fn well_beyond_the_radius_does_not_reach() {
        let radius = 0.025;
        let far_past = radius * 1.01;
        assert_eq!(curved_local_z(far_past, radius), None);
        assert_eq!(curved_local_z(far_past, -radius), None);
    }

    /// Comfortably inside the radius, the tolerance changes nothing.
    #[test]
    fn well_inside_the_radius_is_unaffected() {
        let radius: f64 = 0.025;
        let inside: f64 = 0.015;
        let expected =
            (-radius.signum()).mul_add(radius.mul_add(radius, -(inside * inside)).sqrt(), radius);
        assert_eq!(curved_local_z(inside, radius), Some(expected));
    }
}

#[cfg(test)]
mod test_local_normal {
    use super::*;
    use crate::{error::OpmResult, joule, meter, nanometer};
    use approx::assert_abs_diff_eq;

    /// Half the width of the central difference the tangents are taken over, in meter.
    const STEP: f64 = 1e-6;

    /// A handful of transversal positions out to `reach` meters, to ask a surface about.
    fn transversal_grid(reach: f64) -> Vec<Point2<Length>> {
        vec![
            meter!(0.0, 0.0),
            meter!(reach, 0.0),
            meter!(-reach, 0.0),
            meter!(0.0, reach),
            meter!(0.0, -reach),
            meter!(reach / 2., -reach / 2.),
        ]
    }

    /// A tangent of the surface at `position`, taken as a central difference of
    /// [`GeoSurface::local_z_at`] along `step`.
    fn central_tangent(
        surface: &dyn GeoSurface,
        position: &Point2<Length>,
        step: Vector2<f64>,
    ) -> Vector3<f64> {
        let local_z = |side: f64| {
            surface
                .local_z_at(&Point2::new(
                    position.x + meter!(side * step.x),
                    position.y + meter!(side * step.y),
                ))
                .expect("the surface reaches a small step away as well")
                .value
        };
        Vector3::new(2. * step.x, 2. * step.y, local_z(1.) - local_z(-1.))
    }

    /// Check everything [`GeoSurface::local_normal_at`] promises, at positions the surface reaches
    /// and has not yet turned so far that a finite difference along it becomes unreliable.
    fn check_normal_contract(surface: &dyn GeoSurface, positions: &[Point2<Length>]) {
        let name = surface.name();
        for position in positions {
            let normal = surface
                .local_normal_at(position)
                .expect("the surface reaches this position");
            assert_abs_diff_eq!(normal.norm(), 1.0, epsilon = 1e-12);
            assert!(
                normal.z > 0.0,
                "the normal of '{name}' at {position:?} must face +z, got {normal:?}"
            );
            // The normal has to stand on the surface: walking along the surface in either
            // transversal direction must not get one anywhere along the normal.
            for step in [Vector2::new(STEP, 0.0), Vector2::new(0.0, STEP)] {
                let tangent = central_tangent(surface, position, step).normalize();
                assert_abs_diff_eq!(normal.dot(&tangent), 0.0, epsilon = 1e-8);
            }
            // A ray meeting the same spot has to find the same normal — save for which way it is
            // turned, the very thing that makes it useless for describing a surface.
            let ray = Ray::new_collimated(
                Point3::new(position.x, position.y, meter!(-1.0)),
                nanometer!(1053.0),
                joule!(1.0),
            )
            .expect("a collimated ray along +z");
            let (hit, from_ray) = surface
                .calc_intersect_and_normal(&ray)
                .expect("the ray meets the surface");
            assert_abs_diff_eq!(
                hit.z.value,
                surface
                    .local_z_at(position)
                    .expect("the surface reaches this position")
                    .value,
                epsilon = 1e-9
            );
            assert_abs_diff_eq!(normal.dot(&from_ray).abs(), 1.0, epsilon = 1e-9);
        }
    }

    #[test]
    fn the_normal_of_a_plane_follows_the_contract() {
        check_normal_contract(&Plane, &transversal_grid(0.05));
    }

    #[test]
    fn the_normal_of_a_sphere_follows_the_contract() -> OpmResult<()> {
        for radius in [meter!(0.1), meter!(-0.1)] {
            check_normal_contract(&Sphere::new(radius)?, &transversal_grid(0.08));
        }
        Ok(())
    }

    #[test]
    fn the_normal_of_a_cylinder_follows_the_contract() -> OpmResult<()> {
        for radius in [meter!(0.1), meter!(-0.1)] {
            let cylinder = Cylinder::new(radius)?;
            check_normal_contract(&cylinder, &transversal_grid(0.08));
            // The axis runs along y, so however far along it one goes, the normal stays in the
            // xz plane.
            for along_axis in [meter!(0.0, 0.5), meter!(0.04, -2.0)] {
                let normal = cylinder
                    .local_normal_at(&along_axis)
                    .expect("a cylinder reaches arbitrarily far along its axis");
                assert_abs_diff_eq!(normal.y, 0.0, epsilon = 1e-15);
            }
        }
        Ok(())
    }

    #[test]
    fn the_normal_of_a_parabola_follows_the_contract() -> OpmResult<()> {
        for focal_length in [meter!(0.1), meter!(-0.1)] {
            check_normal_contract(&Parabola::new(focal_length)?, &transversal_grid(0.05));
        }
        Ok(())
    }

    /// Towards the rim a curved surface turns over until it stands upright, and its normal tips out
    /// of the axis with it — by exactly the angle the surface has turned through, `asin(d/R)`.
    #[test]
    fn the_normal_tips_over_towards_the_rim() -> OpmResult<()> {
        let radius = 0.1;
        for surface in [
            Box::new(Sphere::new(meter!(radius))?) as Box<dyn GeoSurface>,
            Box::new(Cylinder::new(meter!(radius))?),
        ] {
            for distance in [0.0, 0.05, 0.0999] {
                let normal = surface
                    .local_normal_at(&meter!(distance, 0.0))
                    .expect("this is still short of the rim");
                assert_abs_diff_eq!(normal.norm(), 1.0, epsilon = 1e-12);
                assert_abs_diff_eq!(normal.x, -distance / radius, epsilon = 1e-12);
                assert_abs_diff_eq!(
                    normal.z,
                    (distance / radius)
                        .mul_add(-(distance / radius), 1.0)
                        .sqrt(),
                    epsilon = 1e-12
                );
            }
        }
        Ok(())
    }

    /// Beyond its radius of curvature a surface has curved back on itself. Neither its position nor
    /// its normal is defined there, and the two have to agree on where that starts.
    #[test]
    fn a_curved_surface_has_no_normal_beyond_its_rim() -> OpmResult<()> {
        let radius = meter!(0.1);
        for surface in [
            Box::new(Sphere::new(radius)?) as Box<dyn GeoSurface>,
            Box::new(Cylinder::new(radius)?),
        ] {
            // Only x is offset: a cylinder still reaches arbitrarily far along its y axis.
            for beyond in [meter!(0.11, 0.0), meter!(-0.2, 0.0)] {
                assert!(surface.local_z_at(&beyond).is_none());
                assert!(surface.local_normal_at(&beyond).is_none());
            }
        }
        Ok(())
    }
}
