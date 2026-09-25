//! Module for handling geometric surfaces
//!
//! This module contains the [`GeoSurface`] trait which handles the interface for calculating things like intersection
//! points etc. and an enum containing the concrete surface types.

use super::Plane;
use crate::{
    error::{OpmResult, OpossumError},
    light::Ray,
    utils::geom_transformation::Isometry,
};
use nalgebra::{Point2, Point3, Vector2, Vector3};
use num_traits::Zero;
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
    /// Return the normal of this [`GeoSurface`] above a given transversal position, in the
    /// surface's own local frame.
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
    /// - `transversal_position`: the position in the local xy plane to look above, given in the
    ///   local frame of this surface.
    ///
    /// # Returns
    ///
    /// The unit normal above the given position, or `None` wherever
    /// [`local_z_at`](GeoSurface::local_z_at) finds no surface either.
    fn local_normal_at(&self, transversal_position: &Point2<Length>) -> Option<Vector3<f64>>;
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

/// How far `half_chord_squared` may dip below zero in [`curved_local_z`] before a position genuinely
/// counts as beyond the surface's curvature, rather than merely landing there because of rounding.
///
/// A hemisphere — curvature radius equal to the aperture radius — is the tightest a spherical or
/// cylindrical surface can be without folding back on itself, and geometrically valid: at the rim,
/// `distance_from_axis` equals `radius` exactly and the sag is zero. But `distance_from_axis` is
/// usually not `radius` bit-for-bit there: it comes from sampling the aperture's outline (points on
/// a circle of that radius, reconstructed through trigonometric functions), whose own rounding can
/// place a rim point a few floating point epsilons beyond it. Squaring that in `half_chord_squared`
/// then yields a result that is negative although the true value is zero, and the surface would
/// reject the very rim of a valid hemisphere as unreachable.
///
/// Relative to `radius²` rather than an absolute distance, so the same tolerance holds whether the
/// surface is measured in micrometres or metres — the same reasoning as [`ALIGNMENT_TOLERANCE`].
/// The value sits far above the rounding this exists for (a few times [`f64::EPSILON`]) and far
/// below any curvature-versus-aperture mismatch a real component would be built with.
const CURVATURE_RIM_TOLERANCE: f64 = 1e-9;

/// Determine the local z position of a curved surface above a given transversal distance from its
/// axis.
///
/// Shared by the surfaces whose local frame is centered on their center of curvature
/// ([`Sphere`](super::Sphere), [`Cylinder`](super::Cylinder)) — the same pair, and for the same
/// reason, as [`is_behind_curvature`]: they differ only in how that distance is measured. The
/// vertex, at distance zero, therefore lies at `-radius` for either sign of the curvature.
///
/// A `distance_from_axis` up to [`CURVATURE_RIM_TOLERANCE`] beyond `radius` is treated as exactly
/// `radius` (see there) — the rim of an exact hemisphere, not a surface asked about a point it
/// cannot reach.
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
    let clamped = if half_chord_squared < 0.0
        && half_chord_squared >= -CURVATURE_RIM_TOLERANCE * radius * radius
    {
        0.0
    } else {
        half_chord_squared
    };
    (clamped >= 0.0).then(|| -radius.signum() * clamped.sqrt())
}

/// Determine the local normal of a curved surface above a given transversal position.
///
/// Shared by the surfaces whose local frame is centered on their center of curvature
/// ([`Sphere`](super::Sphere), [`Cylinder`](super::Cylinder)) — the same pair, and for the same
/// reason, as [`curved_local_z`]: they differ only in which transversal directions bend the
/// surface. The normal runs along the line from the center of curvature to the surface point, which
/// dividing by the *signed* radius turns towards local +z for either sign of the curvature: a
/// convex surface has its center behind it, a concave one in front of it.
///
/// # Arguments
///
/// - `curving`: the transversal position in the directions the surface curves in, in meter — both
///   components for a [`Sphere`](super::Sphere), the x offset alone for a
///   [`Cylinder`](super::Cylinder), whose axis does not curve
/// - `radius`: the signed radius of curvature, in meter
///
/// # Returns
///
/// The unit normal pointing towards local +z, or `None` beyond the radius of curvature, where
/// [`curved_local_z`] finds no surface either.
pub(super) fn curved_local_normal(curving: Vector2<f64>, radius: f64) -> Option<Vector3<f64>> {
    let local_z = curved_local_z(curving.norm(), radius)?;
    Some(Vector3::new(
        -curving.x / radius,
        -curving.y / radius,
        -local_z / radius,
    ))
}

/// How a [`GeoSurface`] stands in a frame that is not its own.
///
/// A surface carries its own placement, but everything built *from* surfaces — the extent of a
/// body, a mesh laid over one — is expressed in the frame of the component instead, which need not
/// agree with it: a wedge tilts its exit surface against the body it bounds, and a decentered lens
/// shifts both of its surfaces sideways. This does that translation once, together with the checks
/// that decide whether the surface can bound that frame at all.
pub(super) struct SurfacePlacement<'a> {
    surface: &'a dyn GeoSurface,
    relative: Isometry,
    axis: Vector3<f64>,
    tilt: f64,
    vertex: Length,
}

/// How far a surface may be out of square with its frame before that counts as a tilt, given as
/// the sine of the angle between the two axes.
///
/// Composing a frame with its own inverse does not come out exactly, so a surface placed square in
/// its node still reports a tilt of a few floating point epsilons — which would otherwise turn
/// every curved surface in a node that is merely rotated into an unsupported one. The threshold
/// sits far above that noise and far below any angle a component is built with: a wedge is turned
/// by degrees, not by billionths of one. The same slack decides when a surface is so nearly
/// parallel to the frame's axis that it no longer bounds anything along it.
const ALIGNMENT_TOLERANCE: f64 = 1e-9;

impl<'a> SurfacePlacement<'a> {
    /// Work out how `surface` stands in `frame`.
    ///
    /// # Arguments
    ///
    /// - `surface`: the surface to place
    /// - `frame`: the frame to measure it against, usually that of an optical node
    ///
    /// # Returns
    ///
    /// The placement, from which the surface can be asked where it lies above a position of the
    /// frame.
    ///
    /// # Errors
    ///
    /// This function returns an error if the surface runs parallel to the frame's axis, so that it
    /// never crosses it and cannot bound anything along it, or if the surface has no vertex.
    pub(super) fn new(surface: &'a dyn GeoSurface, frame: &Isometry) -> OpmResult<Self> {
        let relative = Isometry::new_from_transform(
            frame.get_inv_transform() * surface.isometry().get_transform(),
        );
        // The frame's own z axis, written in the surface's frame: dotted with a point of the
        // surface it yields that point's longitudinal position in the frame. Its transversal part
        // states how far the surface is tilted against the frame, its z part how much of the
        // surface's own sag survives into the frame's z.
        let axis = relative.inverse_transform_vector_f64(&Vector3::z());
        if axis.z.abs() <= ALIGNMENT_TOLERANCE {
            return Err(OpossumError::Other(format!(
                "the '{}' surface runs parallel to the frame's axis and does not bound it",
                surface.name()
            )));
        }
        let vertex = surface.local_z_at(&Point2::origin()).ok_or_else(|| {
            OpossumError::Other(format!(
                "the '{}' surface has no vertex to measure its sag from",
                surface.name()
            ))
        })?;
        Ok(Self {
            surface,
            relative,
            tilt: axis.x.hypot(axis.y),
            axis,
            vertex,
        })
    }
    /// Maps a point of the surface's own frame into the frame it was measured against.
    pub(super) const fn relative(&self) -> &Isometry {
        &self.relative
    }
    /// How far the surface is tilted against the frame's axis, as the sine of that angle.
    pub(super) const fn tilt(&self) -> f64 {
        self.tilt
    }
    /// Whether the surface is turned against the frame's axis at all, rather than square to it.
    ///
    /// See [`ALIGNMENT_TOLERANCE`] for why this is not simply `tilt() > 0.0`.
    pub(super) fn is_tilted(&self) -> bool {
        self.tilt > ALIGNMENT_TOLERANCE
    }
    /// How much of the surface's own sag survives into the frame's z, as the cosine of the tilt.
    ///
    /// Negative if the surface sits the other way round in this frame.
    pub(super) fn alignment(&self) -> f64 {
        self.axis.z
    }
    /// The surface's own anchor point, in the surface's frame.
    pub(super) const fn vertex(&self) -> Length {
        self.vertex
    }
    /// The point of the surface above a transversal position of the frame, and the way it faces
    /// there — both in the frame's coordinates.
    ///
    /// "Above" is along the frame's axis: the point where a line through the given transversal
    /// position, parallel to that axis, meets the surface. Finding it means walking along that line
    /// until the surface's own transversal plane is reached, which is exact in the two cases that
    /// occur: for a surface parallel to the frame the walk does not move transversally at all, so
    /// any sag above the position found is the answer; for a flat one there is no sag to miss. A
    /// surface both tilted *and* curved would be met somewhere the walk does not reach, and is
    /// turned away — the same limit the extent of a body is subject to, and no node builds such a
    /// surface.
    ///
    /// The normal is turned towards the frame's +z, so it is the outward normal of a body this
    /// surface bounds from below and the inward one of a body it bounds from above.
    ///
    /// # Arguments
    ///
    /// - `transversal_position`: the position in the frame's xy plane to look above
    ///
    /// # Returns
    ///
    /// The point of the surface and its unit normal, both in the frame's coordinates.
    ///
    /// # Errors
    ///
    /// This function returns an error if the surface does not reach that far out, or if it is both
    /// curved and tilted against the frame.
    pub(super) fn above(
        &self,
        transversal_position: &Point2<Length>,
    ) -> OpmResult<(Point3<Length>, Vector3<f64>)> {
        let start = self.relative.inverse_transform_point(&Point3::new(
            transversal_position.x,
            transversal_position.y,
            Length::zero(),
        ));
        let along = -start.z / self.alignment();
        let local = Point2::new(start.x + along * self.axis.x, start.y + along * self.axis.y);
        let out_of_reach = || {
            OpossumError::Other(format!(
                "the '{}' surface does not reach as far out as {transversal_position:?}",
                self.surface.name()
            ))
        };
        let surface_z = self.surface.local_z_at(&local).ok_or_else(out_of_reach)?;
        if self.is_tilted() && surface_z != self.vertex {
            return Err(OpossumError::Other(format!(
                "the curved '{}' surface tilted against the frame it is measured in is not \
                 supported",
                self.surface.name()
            )));
        }
        let normal = self.relative.transform_vector_f64(
            &self
                .surface
                .local_normal_at(&local)
                .ok_or_else(out_of_reach)?,
        );
        Ok((
            self.relative
                .transform_point(&Point3::new(local.x, local.y, surface_z)),
            // The surface may sit the other way round in this frame, in which case its own +z is
            // the frame's -z and the normal has to be turned around to keep the promise above.
            if self.alignment().is_sign_negative() {
                -normal
            } else {
                normal
            },
        ))
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

#[cfg(test)]
mod test_curved_local_z {
    use super::*;

    /// Exactly at the radius, a curved surface has curved all the way back onto the transversal
    /// plane it started from: the rim of a hemisphere, sag zero.
    #[test]
    fn exactly_at_the_radius_is_the_rim_with_zero_sag() {
        for radius in [0.025_f64, -0.025] {
            assert_eq!(
                curved_local_z(radius.abs(), radius),
                Some(0.0),
                "radius {radius}"
            );
        }
    }

    /// A distance a few floating point epsilons beyond the radius still counts as the rim - this is
    /// the whole reason [`CURVATURE_RIM_TOLERANCE`] exists: a hemisphere's aperture radius and
    /// curvature radius are set as two independent properties, and even when a user sets them to the
    /// same nominal value, the aperture is reached through a trigonometric sampling of its outline
    /// while the curvature radius is used as given - so the two rarely agree bit-for-bit even where
    /// they are mathematically equal.
    #[test]
    fn a_touch_beyond_the_radius_within_tolerance_still_reaches() {
        let radius = 0.025;
        // A tenth of the tolerance, so the test sits comfortably inside the allowed band rather
        // than on its own edge, where the arithmetic that builds `just_past` would be exposed to
        // exactly the kind of rounding this test is about.
        let just_past = radius * (1.0 + CURVATURE_RIM_TOLERANCE / 10.0);
        assert_eq!(curved_local_z(just_past, radius), Some(0.0));
        assert_eq!(curved_local_z(just_past, -radius), Some(0.0));
    }

    /// A distance genuinely beyond the radius - orders of magnitude past anything rounding could
    /// produce - still does not reach: the tolerance is not a loosening of the check, only a
    /// narrow allowance for where the surface already is.
    #[test]
    fn well_beyond_the_radius_does_not_reach() {
        let radius = 0.025;
        let far_past = radius * 1.01;
        assert_eq!(curved_local_z(far_past, radius), None);
        assert_eq!(curved_local_z(far_past, -radius), None);
    }

    /// Comfortably inside the radius, the tolerance changes nothing: the sag is the same value the
    /// plain formula would give.
    #[test]
    fn well_inside_the_radius_is_unaffected() {
        let radius: f64 = 0.025;
        let inside: f64 = 0.015;
        let expected = -radius.signum() * radius.mul_add(radius, -(inside * inside)).sqrt();
        assert_eq!(curved_local_z(inside, radius), Some(expected));
    }
}

#[cfg(test)]
mod test_local_normal {
    use super::*;
    use crate::{
        error::OpmResult,
        geometry::{Cylinder, Parabola, Sphere},
        joule, meter, nanometer,
    };
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
                .calc_intersect_and_normal_do(&ray)
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
        check_normal_contract(&Plane::default(), &transversal_grid(0.05));
    }

    #[test]
    fn the_normal_of_a_sphere_follows_the_contract() -> OpmResult<()> {
        for radius in [meter!(0.1), meter!(-0.1)] {
            check_normal_contract(
                &Sphere::new(radius, Isometry::identity())?,
                &transversal_grid(0.08),
            );
        }
        Ok(())
    }

    #[test]
    fn the_normal_of_a_cylinder_follows_the_contract() -> OpmResult<()> {
        for radius in [meter!(0.1), meter!(-0.1)] {
            let cylinder = Cylinder::new(radius, Isometry::identity())?;
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
            check_normal_contract(
                &Parabola::new(focal_length, Isometry::identity())?,
                &transversal_grid(0.05),
            );
        }
        Ok(())
    }

    /// Towards the rim a curved surface turns over until it stands upright, and its normal tips out
    /// of the axis with it — by exactly the angle the surface has turned through, `asin(d/R)`.
    ///
    /// Note that the rim itself is not asked about: there the surface stands upright, and whether
    /// that last point still belongs to it is a question floating point cannot settle — see
    /// [`curved_local_z`], whose fused multiply-add lands a hair either side of zero.
    #[test]
    fn the_normal_tips_over_towards_the_rim() -> OpmResult<()> {
        let radius = 0.1;
        for surface in [
            Box::new(Sphere::new(meter!(radius), Isometry::identity())?) as Box<dyn GeoSurface>,
            Box::new(Cylinder::new(meter!(radius), Isometry::identity())?),
        ] {
            for distance in [0.0, 0.05, 0.0999] {
                let normal = surface
                    .local_normal_at(&meter!(distance, 0.0))
                    .expect("this is still short of the rim");
                assert_abs_diff_eq!(normal.norm(), 1.0, epsilon = 1e-12);
                // sin and cos of that angle, stated directly rather than through an inverse
                // trigonometric function, which is ill-conditioned near the axis.
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
            Box::new(Sphere::new(radius, Isometry::identity())?) as Box<dyn GeoSurface>,
            Box::new(Cylinder::new(radius, Isometry::identity())?),
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
