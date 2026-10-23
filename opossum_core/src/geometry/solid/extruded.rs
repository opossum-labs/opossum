#![warn(missing_docs)]
//! Two faces capping a profile that is extruded along an axis: lens, cylindric lens, wedge, plate.
//!
//! This file holds the solid ([`Extruded`]) and what it knows about its volume: whether a point lies
//! inside it, how far a ray travels through it and which box encloses it (see [`Body`]).

use nalgebra::{Point2, Point3, Vector3};
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use std::ops::Range;
use uom::si::f64::Length;

use super::{Side, face_hits};
use crate::{
    apertures::Outline,
    error::{OpmResult, OpossumError},
    gain::Inversion,
    geometry::{
        Face, FaceIndex, GeometryKind, Rim,
        body::{Body, BoundingBox},
    },
    light::Ray,
    utils::geom_transformation::Isometry,
};

/// Two faces capping a profile that is extruded along an axis.
///
/// The profile is the outline of the [`Rim`], stated in the plane perpendicular to the rim's axis;
/// it is the component's optic shape. The center thickness is not stored separately: it is where
/// the rear face's vertex sits, together with any tilt of that face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extruded {
    rim: Rim,
    front: Face,
    rear: Face,
    #[serde(skip)]
    inversion: Option<Inversion>,
}

impl Extruded {
    /// Create a new [`Extruded`] solid.
    ///
    /// # Arguments
    ///
    /// * `axis` - the frame of the extrusion relative to the node; its z axis is the extrusion
    ///   direction. The identity for a component used along its own axis, such as a lens.
    /// * `front` - the face capping the profile towards -z, stated in the `axis` frame.
    /// * `rear` - the face capping the profile towards +z, stated in the `axis` frame.
    /// * `outline` - the profile, in the xy plane of the `axis` frame.
    ///
    /// # Errors
    ///
    /// This function returns an error if a face is curved too tightly to reach the edge of the
    /// profile.
    pub fn new(
        axis: Isometry,
        mut front: Face,
        mut rear: Face,
        outline: Outline,
    ) -> OpmResult<Self> {
        let reach = outline.transversal_reach();
        front.check_reach(reach, "front face")?;
        rear.check_reach(reach, "rear face")?;
        front.move_by(&axis);
        rear.move_by(&axis);
        Ok(Self {
            rim: Rim::new(outline, axis),
            front,
            rear,
            inversion: None,
        })
    }
    /// Return the rim bounding this solid sideways: its profile and the axis it is extruded along.
    #[must_use]
    pub const fn rim(&self) -> &Rim {
        &self.rim
    }
    /// The number of the face capping the profile towards -z.
    pub const FRONT: FaceIndex = FaceIndex::new(0);
    /// The number of the face capping the profile towards +z.
    pub const REAR: FaceIndex = FaceIndex::new(1);
    /// Return the face with the given number and the side of it the profile lies on: behind the
    /// front face and in front of the rear face.
    const fn face_and_side(&self, index: FaceIndex) -> Option<(&Face, Side)> {
        match index {
            Self::FRONT => Some((&self.front, Side::Behind)),
            Self::REAR => Some((&self.rear, Side::InFront)),
            _ => None,
        }
    }
}

/// Both faces end at the rim of the profile, on the sheet through their vertex.
impl GeometryKind for Extruded {
    fn face(&self, index: FaceIndex) -> Option<&Face> {
        self.face_and_side(index).map(|(face, _)| face)
    }
    fn face_count(&self) -> usize {
        2
    }
    fn includes(&self, face: FaceIndex, point: &Point3<Length>) -> bool {
        GeometryKind::face(self, face).is_some_and(|surface| self.rim.bounds(surface, point))
    }
    fn window(&self) -> Option<&Rim> {
        None
    }
    fn as_body(&self) -> Option<&dyn Body> {
        Some(self)
    }
    fn as_body_mut(&mut self) -> Option<&mut dyn Body> {
        Some(self)
    }
    fn move_by(&mut self, by: &Isometry) {
        self.rim.move_by(by);
        self.front.move_by(by);
        self.rear.move_by(by);
    }
}

impl Extruded {
    /// Return the range of longitudinal positions one of the bounding surfaces spans over the cross
    /// section of this body, in the body's own frame.
    ///
    /// The surface is asked where it lies above its own axis — its vertex — and how far it deviates
    /// from that towards the rim
    /// ([`GeoSurface::local_z_at`](crate::geometry::GeoSurface::local_z_at)). Both
    /// answers are given in the surface's own frame, which need not be the body's: a wedge tilts its
    /// exit surface against the body it bounds.
    ///
    /// The result is exact for a surface that is either flat or aligned with the body's axis, which
    /// is every surface the volume nodes build. For one that is both curved *and* tilted, the
    /// transversal reach derived below neglects that the sag itself tilts out of the cross section,
    /// which would make the range slightly too narrow — such a surface is therefore rejected rather
    /// than silently truncating the body. A tilt as small as the rounding noise of placing the
    /// surfaces does not count as one.
    ///
    /// # Arguments
    ///
    /// - `face`: the bounding face to measure
    /// - `transversal_reach`: the largest distance from the body's axis its cross section reaches
    ///
    /// # Returns
    ///
    /// The longitudinal range the surface spans over the body's cross section.
    ///
    /// # Errors
    ///
    /// This function returns an error if the surface runs parallel to the body's axis, if it does
    /// not reach as far out as the cross section, or if it is both curved and tilted against the
    /// body.
    fn surface_z_range(&self, face: &Face, transversal_reach: Length) -> OpmResult<Range<Length>> {
        // Placing a surface and its body leaves rounding noise in their relative orientation.
        const ORIENTATION_NOISE: f64 = 1.0e-12;
        let surface = face.surface();
        let relative = Isometry::new_from_transform(
            self.isometry().get_inv_transform() * face.frame().get_transform(),
        );
        // The body's own z axis, written in the surface's frame: dotted with a point of the
        // surface it yields that point's longitudinal position in the body. Its transversal part
        // states how far the surface is tilted against the body, its z part how much of the
        // surface's own sag survives into the body's z.
        let body_axis = relative.inverse_transform_vector_f64(&Vector3::z());
        let tilt = body_axis.x.hypot(body_axis.y);
        let alignment = body_axis.z;
        if alignment.abs() < ORIENTATION_NOISE {
            return Err(OpossumError::Other(format!(
                "the '{}' surface runs parallel to the body's axis and does not bound it",
                surface.name()
            )));
        }
        // A tilted surface has to be looked at further out to still span the cross section.
        let reach = transversal_reach / alignment.abs();
        let out_of_reach = || {
            OpossumError::Other(format!(
                "the '{}' surface does not reach as far out as the cross section of the body",
                surface.name()
            ))
        };
        let vertex = surface
            .local_z_at(&Point2::origin())
            .ok_or_else(out_of_reach)?;
        // The sag grows monotonically with the distance from the axis for every surface modelled
        // here, so its extremes over the cross section are attained at the rim. Both axes are
        // sampled because a Cylinder is curved along one of them only.
        let mut lowest_sag = Length::zero();
        let mut highest_sag = Length::zero();
        for rim in [
            Point2::new(reach, Length::zero()),
            Point2::new(-reach, Length::zero()),
            Point2::new(Length::zero(), reach),
            Point2::new(Length::zero(), -reach),
        ] {
            let sag = surface.local_z_at(&rim).ok_or_else(out_of_reach)? - vertex;
            lowest_sag = Length::min(lowest_sag, sag);
            highest_sag = Length::max(highest_sag, sag);
        }
        if tilt > ORIENTATION_NOISE
            && (lowest_sag != Length::zero() || highest_sag != Length::zero())
        {
            return Err(OpossumError::Other(format!(
                "the extent of a body bounded by the curved '{}' surface tilted against it is not \
                 supported",
                surface.name()
            )));
        }
        let anchor = relative
            .transform_point(&Point3::new(Length::zero(), Length::zero(), vertex))
            .z;
        let from_tilt = reach * tilt;
        let (one_sag, other_sag) = (lowest_sag * alignment, highest_sag * alignment);
        Ok(anchor - from_tilt + Length::min(one_sag, other_sag)
            ..anchor + from_tilt + Length::max(one_sag, other_sag))
    }
}

/// A point is inside this body if it lies behind the front surface (see
/// [`is_behind`](crate::geometry::GeoSurface::is_behind)), not behind the rear surface
/// and within the [`Rim`](crate::geometry::Rim) of the solid. Both bounding surfaces are the faces
/// a node traces its rays at.
///
/// **Note**: The lateral boundary is the rim alone — it is not a surface rays can interact with. A
/// ray leaving the volume sideways is therefore reported as not passing through the body rather
/// than being reflected on a barrel surface.
impl Body for Extruded {
    fn contains(&self, point: &Point3<Length>) -> bool {
        self.rim.contains(point) && self.front.is_behind(point) && !self.rear.is_behind(point)
    }
    fn path_length_inside(&self, ray: &Ray) -> Option<Length> {
        // A ray leaving the body through its lateral boundary meets no hit there: that boundary is
        // not part of the model.
        let hits = face_hits([&self.front, &self.rear].into_iter(), ray, |face, point| {
            self.includes(face, point)
        });
        self.chord_through(ray.position(), ray.direction(), hits)
    }
    fn bounding_box(&self) -> OpmResult<BoundingBox> {
        let outline = self.rim.outline();
        let extremes = outline.extreme_points();
        let (Some(x_range), Some(y_range)) = (
            span(extremes.iter().map(|point| point.x)),
            span(extremes.iter().map(|point| point.y)),
        ) else {
            return Err(OpossumError::Other(
                "the outline of the body has no extent at all".into(),
            ));
        };
        let transversal_reach = outline.transversal_reach();
        let front_z = self.surface_z_range(&self.front, transversal_reach)?;
        let rear_z = self.surface_z_range(&self.rear, transversal_reach)?;
        // Which of the two bounds the body from below is not assumed here: a turned solid
        // encloses the same volume as an upright one, so the two are simply united.
        BoundingBox::new(
            Point3::new(
                x_range.start,
                y_range.start,
                Length::min(front_z.start, rear_z.start),
            ),
            Point3::new(
                x_range.end,
                y_range.end,
                Length::max(front_z.end, rear_z.end),
            ),
        )
    }
    fn isometry(&self) -> Isometry {
        *self.rim.frame()
    }
    fn side(&self, face: FaceIndex) -> Option<Side> {
        self.face_and_side(face).map(|(_, side)| side)
    }
    fn inversion(&self) -> Option<&Inversion> {
        self.inversion.as_ref()
    }
    fn inversion_mut(&mut self) -> &mut Option<Inversion> {
        &mut self.inversion
    }
}

/// Return the smallest and the largest of the given lengths.
///
/// # Arguments
///
/// - `values`: the lengths to be spanned
///
/// # Returns
///
/// The range from the smallest to the largest value, or `None` if there are no values at all.
fn span(values: impl Iterator<Item = Length>) -> Option<Range<Length>> {
    values.fold(None, |spanned: Option<Range<Length>>, value| {
        Some(spanned.map_or(value..value, |range| {
            Length::min(range.start, value)..Length::max(range.end, value)
        }))
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Bore, CircleShape, RectangleShape},
        degree,
        geometry::{Cylinder, Face, Sphere, SurfaceShape},
        joule, millimeter, nanometer,
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::Vector3;
    use uom::si::{f64::Angle, length::millimeter};

    /// Create a circular outline of the given radius.
    fn circular_cross_section(radius: Length) -> OpmResult<Outline> {
        Outline::new(CircleShape::new(radius)?, Vec::new())
    }
    /// Create the two flat faces of a plate of the given thickness.
    fn plate_faces(thickness: Length) -> OpmResult<(Face, Face)> {
        Ok((
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Face::new(SurfaceShape::Plane, Isometry::new_along_z(thickness)?),
        ))
    }
    /// Create a spherical face of the given radius of curvature, its vertex at the given position
    /// along z.
    fn sphere_face(radius: Length, vertex_z: Length) -> OpmResult<Face> {
        Ok(Face::new(
            SurfaceShape::Sphere(Sphere::new(radius)?),
            Isometry::new_along_z(vertex_z)?,
        ))
    }
    /// Create the body between two faces, extruded along the node's own axis, at the given node
    /// frame.
    fn body_between(
        front: Face,
        rear: Face,
        outline: Outline,
        node_frame: Isometry,
    ) -> OpmResult<Extruded> {
        let mut body = Extruded::new(Isometry::identity(), front, rear, outline)?;
        body.move_by(&node_frame);
        Ok(body)
    }
    /// Create a disk of the given thickness and radius, with its entrance surface at z = 0.
    fn disk(thickness: Length, radius: Length) -> OpmResult<Extruded> {
        let (front, rear) = plate_faces(thickness)?;
        body_between(
            front,
            rear,
            circular_cross_section(radius)?,
            Isometry::identity(),
        )
    }
    /// Create a ray at the given position, propagating along the given direction.
    fn test_ray(position: Point3<Length>, direction: Vector3<f64>) -> OpmResult<Ray> {
        Ray::new(position, direction, nanometer!(1053.0), joule!(1.0))
    }
    /// Trace the given ray through the given body and require it to pass through.
    fn path_length(body: &Extruded, ray: &Ray) -> OpmResult<Length> {
        body.path_length_inside(ray)
            .ok_or_else(|| crate::error::OpossumError::Other("ray missed the body".into()))
    }
    /// Both faces end at the rim of the profile; a closed surface also only on the sheet through its
    /// vertex, not on the far side of its equator.
    #[test]
    fn the_faces_end_at_the_rim_and_on_the_sheet_through_the_vertex() -> OpmResult<()> {
        let (_, rear) = plate_faces(millimeter!(30.0))?;
        let body = body_between(
            sphere_face(millimeter!(15.0), millimeter!(0.0))?,
            rear,
            circular_cross_section(millimeter!(12.5))?,
            Isometry::identity(),
        )?;
        // The points lie on the sphere: the sheet check allows a nanometer.
        let on_sphere = |x: f64| 15.0 - 15.0_f64.mul_add(15.0, -(x * x)).sqrt();
        for (face, (x, y, z), included) in [
            (Extruded::FRONT, (0.0, 0.0, 0.0), true),
            (Extruded::FRONT, (12.4, 0.0, on_sphere(12.4)), true),
            (Extruded::FRONT, (12.6, 0.0, on_sphere(12.6)), false),
            (Extruded::REAR, (12.4, 0.0, 30.0), true),
            (Extruded::REAR, (0.0, -12.6, 30.0), false),
            // There is no third face.
            (FaceIndex::new(2), (0.0, 0.0, 0.0), false),
        ] {
            assert_eq!(
                body.includes(face, &millimeter!(x, y, z)),
                included,
                "face {face} at ({x}, {y}, {z})"
            );
        }
        // 135° from the vertex of the sphere: behind its equator, 10.6 mm off the axis and so within
        // the rim when seen along the axis.
        let polar = 135.0_f64.to_radians();
        let far_sheet = millimeter!(15.0 * polar.sin(), 0.0, 15.0 - 15.0 * polar.cos());
        assert!(!body.includes(Extruded::FRONT, &far_sheet));
        Ok(())
    }
    #[test]
    fn contains_disk() -> OpmResult<()> {
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        assert!(body.contains(&millimeter!(0.0, 0.0, 5.0)));
        // the body is half-open: the entrance surface belongs to it, the exit surface does not
        assert!(body.contains(&millimeter!(0.0, 0.0, 0.0)));
        assert!(!body.contains(&millimeter!(0.0, 0.0, 10.0)));
        // in front of the entrance surface / behind the exit surface
        assert!(!body.contains(&millimeter!(0.0, 0.0, -0.1)));
        assert!(!body.contains(&millimeter!(0.0, 0.0, 10.1)));
        // outside the transversal cross section
        assert!(body.contains(&millimeter!(4.9, 0.0, 5.0)));
        assert!(!body.contains(&millimeter!(5.1, 0.0, 5.0)));
        assert!(!body.contains(&millimeter!(0.0, -5.1, 5.0)));
        Ok(())
    }
    #[test]
    fn contains_slab() -> OpmResult<()> {
        // a slab is the same body with a rectangular instead of a circular cross section
        let (front, rear) = plate_faces(millimeter!(10.0))?;
        let body = body_between(
            front,
            rear,
            Outline::new(
                RectangleShape::new(millimeter!(20.0), millimeter!(4.0))?,
                Vec::new(),
            )?,
            Isometry::identity(),
        )?;
        assert!(body.contains(&millimeter!(9.0, 1.9, 5.0)));
        assert!(!body.contains(&millimeter!(11.0, 1.9, 5.0)));
        assert!(!body.contains(&millimeter!(9.0, 2.1, 5.0)));
        Ok(())
    }
    #[test]
    fn contains_lens() -> OpmResult<()> {
        // a biconvex lens: both centers of curvature lie inside the body
        let body = body_between(
            sphere_face(millimeter!(50.0), millimeter!(0.0))?,
            sphere_face(millimeter!(-50.0), millimeter!(10.0))?,
            circular_cross_section(millimeter!(10.0))?,
            Isometry::identity(),
        )?;
        assert!(body.contains(&millimeter!(0.0, 0.0, 5.0)));
        assert!(!body.contains(&millimeter!(0.0, 0.0, -0.1)));
        assert!(!body.contains(&millimeter!(0.0, 0.0, 10.1)));
        // the sag at x = 10 mm is 1.0102 mm, so the body is thinner towards its rim
        assert!(body.contains(&millimeter!(10.0, 0.0, 5.0)));
        assert!(!body.contains(&millimeter!(10.0, 0.0, 1.0)));
        assert!(!body.contains(&millimeter!(10.0, 0.0, 9.0)));
        Ok(())
    }
    #[test]
    fn path_length_on_axis() -> OpmResult<()> {
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let ray = test_ray(millimeter!(0.0, 0.0, -20.0), Vector3::z())?;
        let path_length = path_length(&body, &ray)?;
        assert_abs_diff_eq!(path_length.value, millimeter!(10.0).value);
        Ok(())
    }
    #[test]
    fn path_length_oblique() -> OpmResult<()> {
        // a ray at 45 degrees travels sqrt(2) times the thickness through the disk
        let body = disk(millimeter!(10.0), millimeter!(20.0))?;
        let ray = test_ray(millimeter!(0.0, -5.0, -5.0), Vector3::new(0.0, 1.0, 1.0))?;
        let path_length = path_length(&body, &ray)?;
        assert_abs_diff_eq!(
            path_length.value,
            millimeter!(10.0 * f64::sqrt(2.0)).value,
            epsilon = 1e-12
        );
        Ok(())
    }
    #[test]
    fn path_length_starting_on_the_entrance_surface() -> OpmResult<()> {
        // a ray refracted at the entrance surface starts exactly on it, so it hits that surface at
        // zero distance. That hit must not be mistaken for the point where it leaves the body.
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let ray = test_ray(millimeter!(0.0, 0.0, 0.0), Vector3::z())?;
        let path_length = path_length(&body, &ray)?;
        assert_abs_diff_eq!(path_length.value, millimeter!(10.0).value);
        Ok(())
    }
    #[test]
    fn path_length_starting_on_the_exit_surface() -> OpmResult<()> {
        // the same for a ray entering the body backwards through its exit surface
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let ray = test_ray(millimeter!(0.0, 0.0, 10.0), -Vector3::z())?;
        let path_length = path_length(&body, &ray)?;
        assert_abs_diff_eq!(path_length.value, millimeter!(10.0).value);
        Ok(())
    }
    #[test]
    fn path_length_starting_inside() -> OpmResult<()> {
        // a ray starting inside the body only travels the remaining distance to the exit surface
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let ray = test_ray(millimeter!(0.0, 0.0, 4.0), Vector3::z())?;
        let path_length = path_length(&body, &ray)?;
        assert_abs_diff_eq!(path_length.value, millimeter!(6.0).value);
        Ok(())
    }
    #[test]
    fn path_length_backwards() -> OpmResult<()> {
        // a backwards propagating ray leaves the body through its entrance surface
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let ray = test_ray(millimeter!(0.0, 0.0, 4.0), -Vector3::z())?;
        let path_length = path_length(&body, &ray)?;
        assert_abs_diff_eq!(path_length.value, millimeter!(4.0).value);
        Ok(())
    }
    #[test]
    fn path_length_missing_the_body() -> OpmResult<()> {
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        // passing by outside the aperture
        let ray = test_ray(millimeter!(6.0, 0.0, -20.0), Vector3::z())?;
        assert_eq!(body.path_length_inside(&ray), None);
        // starting behind the body
        let ray = test_ray(millimeter!(0.0, 0.0, 20.0), Vector3::z())?;
        assert_eq!(body.path_length_inside(&ray), None);
        Ok(())
    }
    /// A hemisphere: a sphere of 15 mm radius with its vertex at the origin, cut flat at its
    /// equator and bounded to 12.5 mm around the axis. A ray parallel to the axis at height h meets
    /// the sphere sqrt(R² - h²) before the cut. A steep ray reaching the sphere only behind its
    /// equator, beyond the cut, does not pass through the body at all.
    #[test]
    fn chords_through_a_hemisphere() -> OpmResult<()> {
        let radius = 15.0;
        let body = body_between(
            sphere_face(millimeter!(radius), millimeter!(0.0))?,
            Face::new(
                SurfaceShape::Plane,
                Isometry::new_along_z(millimeter!(radius))?,
            ),
            circular_cross_section(millimeter!(12.5))?,
            Isometry::identity(),
        )?;
        for height in [0.0, 10.0] {
            let ray = test_ray(millimeter!(0.0, height, -10.0), Vector3::z())?;
            assert_abs_diff_eq!(
                path_length(&body, &ray)?.value,
                millimeter!(f64::sqrt(radius.mul_add(radius, -height * height))).value,
                epsilon = 1e-12
            );
        }
        // 135° from the vertex, 10.6 mm off the axis - within the rim, but behind the equator.
        let (polar, tilt) = (135.0_f64.to_radians(), 50.0_f64.to_radians());
        let behind = (radius * polar.sin(), radius - radius * polar.cos());
        let direction = Vector3::new(-tilt.sin(), 0.0, tilt.cos());
        let steep = test_ray(
            millimeter!(
                behind.0 - 30.0 * direction.x,
                0.0,
                behind.1 - 30.0 * direction.z
            ),
            direction,
        )?;
        assert_eq!(body.path_length_inside(&steep), None);
        Ok(())
    }
    /// Assert that the given range runs between the two given positions, in millimeter.
    fn assert_spans(range: &Range<Length>, start: f64, end: f64) {
        assert_abs_diff_eq!(range.start.value, millimeter!(start).value, epsilon = 1e-9);
        assert_abs_diff_eq!(range.end.value, millimeter!(end).value, epsilon = 1e-9);
    }
    #[test]
    fn bounding_box_of_a_disk() -> OpmResult<()> {
        // Both bounding surfaces are flat and the cross section is centered, so the box is the
        // body: the plate thickness in z, the aperture radius transversally.
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let bounds = body.bounding_box()?;
        assert_spans(&bounds.x_range(), -5.0, 5.0);
        assert_spans(&bounds.y_range(), -5.0, 5.0);
        assert_spans(&bounds.z_range(), 0.0, 10.0);
        Ok(())
    }
    #[test]
    fn bounding_box_of_a_lens() -> OpmResult<()> {
        // The same biconvex lens as `contains_lens`: its two vertices are the extreme points along
        // z, since both surfaces curve *into* the body from there. The sag at the rim is 1.0102 mm,
        // so a box derived from the surfaces without their curvature would be that much too large.
        let body = body_between(
            sphere_face(millimeter!(50.0), millimeter!(0.0))?,
            sphere_face(millimeter!(-50.0), millimeter!(10.0))?,
            circular_cross_section(millimeter!(10.0))?,
            Isometry::identity(),
        )?;
        let bounds = body.bounding_box()?;
        assert_spans(&bounds.x_range(), -10.0, 10.0);
        assert_spans(&bounds.y_range(), -10.0, 10.0);
        assert_spans(&bounds.z_range(), 0.0, 10.0);
        Ok(())
    }
    #[test]
    fn bounding_box_of_a_wedge() -> OpmResult<()> {
        // A wedge tilts its exit surface against the body, so that surface alone reaches further
        // than the center thickness in both directions - by the aperture radius times the tangent
        // of the wedge angle.
        let angle = degree!(30.0);
        let exit_iso = Isometry::new_along_z(millimeter!(10.0))?.append(&Isometry::new(
            Point3::origin(),
            Point3::new(angle, Angle::zero(), Angle::zero()),
        )?);
        let body = body_between(
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Face::new(SurfaceShape::Plane, exit_iso),
            circular_cross_section(millimeter!(5.0))?,
            Isometry::identity(),
        )?;
        let overhang = 5.0 * angle.value.tan();
        let bounds = body.bounding_box()?;
        assert_spans(&bounds.x_range(), -5.0, 5.0);
        assert_spans(&bounds.y_range(), -5.0, 5.0);
        assert_spans(&bounds.z_range(), 0.0, 10.0 + overhang);
        Ok(())
    }
    #[test]
    fn bounding_box_of_a_rectangular_cross_section() -> OpmResult<()> {
        let (front, rear) = plate_faces(millimeter!(10.0))?;
        let body = body_between(
            front,
            rear,
            Outline::new(
                RectangleShape::new(millimeter!(20.0), millimeter!(4.0))?,
                Vec::new(),
            )?,
            Isometry::identity(),
        )?;
        let bounds = body.bounding_box()?;
        assert_spans(&bounds.x_range(), -10.0, 10.0);
        assert_spans(&bounds.y_range(), -2.0, 2.0);
        Ok(())
    }
    /// A ring (a plate with a central bore) reaches as far as its outer circle. A bore only takes
    /// away from the plate, so one beside the axis does not change the box either.
    #[test]
    fn bounding_box_of_a_ring_is_that_of_its_outer_circle() -> OpmResult<()> {
        let bounds_of_disk_with = |bore: Bore| -> OpmResult<BoundingBox> {
            let (front, rear) = plate_faces(millimeter!(10.0))?;
            let outline = Outline::new(CircleShape::new(millimeter!(8.0))?, vec![bore])?;
            body_between(front, rear, outline, Isometry::identity())?.bounding_box()
        };
        let bore = |x: f64, y: f64| -> OpmResult<Bore> {
            Ok(Bore::new(
                CircleShape::new(millimeter!(3.0))?,
                millimeter!(x, y),
                Angle::zero(),
            ))
        };
        for bore in [bore(0.0, 0.0)?, bore(0.0, 4.0)?] {
            let bounds = bounds_of_disk_with(bore)?;
            assert_spans(&bounds.x_range(), -8.0, 8.0);
            assert_spans(&bounds.y_range(), -8.0, 8.0);
            assert_spans(&bounds.z_range(), 0.0, 10.0);
        }
        Ok(())
    }
    #[test]
    fn bounding_box_of_a_body_rotated_about_its_own_axis() -> OpmResult<()> {
        // A cylindric lens rotates its curved surface about the optical axis to choose the
        // direction it focuses in. That is not a tilt against the body, so the box stays exact.
        let curved_iso = Isometry::new(
            Point3::origin(),
            Point3::new(Angle::zero(), Angle::zero(), degree!(90.0)),
        )?;
        let body = body_between(
            Face::new(
                SurfaceShape::Cylinder(Cylinder::new(millimeter!(50.0))?),
                curved_iso,
            ),
            Face::new(
                SurfaceShape::Plane,
                Isometry::new_along_z(millimeter!(10.0))?,
            ),
            circular_cross_section(millimeter!(10.0))?,
            Isometry::identity(),
        )?;
        let bounds = body.bounding_box()?;
        // same sag as the spherical lens above, since the radius and the aperture are the same
        assert_spans(&bounds.z_range(), 0.0, 10.0);
        assert_abs_diff_eq!(
            bounds.z_range().end.value - bounds.z_range().start.value,
            millimeter!(10.0).value,
            epsilon = 1e-9
        );
        Ok(())
    }
    #[test]
    fn bounding_box_rejects_a_curved_surface_tilted_against_the_body() -> OpmResult<()> {
        // For such a surface the sag itself tilts out of the cross section, which the transversal
        // reach below does not account for - so the box would come out too small. That is refused
        // rather than silently truncating the body.
        let tilted_iso = Isometry::new(
            Point3::origin(),
            Point3::new(degree!(30.0), Angle::zero(), Angle::zero()),
        )?;
        let body = body_between(
            Face::new(
                SurfaceShape::Cylinder(Cylinder::new(millimeter!(50.0))?),
                tilted_iso,
            ),
            Face::new(
                SurfaceShape::Plane,
                Isometry::new_along_z(millimeter!(10.0))?,
            ),
            circular_cross_section(millimeter!(10.0))?,
            Isometry::identity(),
        )?;
        assert!(body.bounding_box().is_err());
        Ok(())
    }
    #[test]
    fn bounding_box_rejects_a_tilted_surface_that_does_not_reach_the_cross_section() -> OpmResult<()>
    {
        // `Extruded::new` already refuses a face that ends before the cross section does, but a
        // tilted face has to be looked at further out to span it: 10 mm across the body is
        // 10 mm / cos 50° = 15.6 mm along the sphere of 12 mm radius, which does not reach that far.
        let tilted_iso = Isometry::new(
            Point3::origin(),
            Point3::new(degree!(50.0), Angle::zero(), Angle::zero()),
        )?;
        let body = body_between(
            Face::new(
                SurfaceShape::Sphere(Sphere::new(millimeter!(12.0))?),
                tilted_iso,
            ),
            Face::new(
                SurfaceShape::Plane,
                Isometry::new_along_z(millimeter!(10.0))?,
            ),
            circular_cross_section(millimeter!(10.0))?,
            Isometry::identity(),
        )?;
        let error = body.bounding_box().unwrap_err();
        assert!(error.to_string().contains("does not reach"), "{error}");
        Ok(())
    }
    #[test]
    fn bounding_box_rejects_a_surface_parallel_to_the_axis() -> OpmResult<()> {
        // Turned by 90 degrees the exit plane bounds nothing, even though the rounded cosine of
        // that turn is not exactly zero.
        let parallel = Isometry::new(millimeter!(0.0, 0.0, 10.0), degree!(90.0, 0.0, 0.0))?;
        let body = body_between(
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Face::new(SurfaceShape::Plane, parallel),
            circular_cross_section(millimeter!(5.0))?,
            Isometry::identity(),
        )?;
        let error = body.bounding_box().unwrap_err();
        assert!(error.to_string().contains("runs parallel"), "{error}");
        Ok(())
    }
    #[test]
    fn bounding_box_is_expressed_in_the_body_frame() -> OpmResult<()> {
        // Moving and tilting the whole body must not change its box: it is stated in the body's own
        // frame, which is exactly what lets a grid over it follow the component.
        let placement = Isometry::new(
            millimeter!(20.0, -5.0, 100.0),
            Point3::new(degree!(15.0), degree!(-10.0), Angle::zero()),
        )?;
        let plate_at = |node_frame: Isometry| -> OpmResult<Extruded> {
            let (front, rear) = plate_faces(millimeter!(10.0))?;
            body_between(
                front,
                rear,
                circular_cross_section(millimeter!(5.0))?,
                node_frame,
            )
        };
        let (placed_bounds, upright_bounds) = (
            plate_at(placement)?.bounding_box()?,
            plate_at(Isometry::identity())?.bounding_box()?,
        );
        for (placed_range, upright_range) in [
            (placed_bounds.x_range(), upright_bounds.x_range()),
            (placed_bounds.y_range(), upright_bounds.y_range()),
            (placed_bounds.z_range(), upright_bounds.z_range()),
        ] {
            assert_spans(
                &placed_range,
                upright_range.start.get::<millimeter>(),
                upright_range.end.get::<millimeter>(),
            );
        }
        Ok(())
    }
    /// A placed, tilted and aligned lens has the box of an upright one.
    ///
    /// Regression test: placing a curved body leaves rounding noise as tilt between its surfaces and
    /// its frame, and that noise was mistaken for a curved surface tilted against the body.
    #[test]
    fn a_placed_curved_body_has_the_bounding_box_of_an_upright_one() -> OpmResult<()> {
        let lens_at = |placement: Isometry| -> OpmResult<Extruded> {
            body_between(
                sphere_face(millimeter!(50.0), millimeter!(0.0))?,
                sphere_face(millimeter!(-50.0), millimeter!(10.0))?,
                circular_cross_section(millimeter!(10.0))?,
                placement,
            )
        };
        let placement = Isometry::new(
            millimeter!(20.0, -5.0, 100.0),
            Point3::new(degree!(15.0), degree!(-10.0), Angle::zero()),
        )?
        .append(&Isometry::new(
            millimeter!(1.0, -2.0, 0.0),
            degree!(3.0, -2.0, 0.0),
        )?);
        let placed = lens_at(placement)?.bounding_box()?;
        let upright = lens_at(Isometry::identity())?.bounding_box()?;
        for (placed_range, upright_range) in [
            (placed.x_range(), upright.x_range()),
            (placed.y_range(), upright.y_range()),
            (placed.z_range(), upright.z_range()),
        ] {
            assert_spans(
                &placed_range,
                upright_range.start.get::<millimeter>(),
                upright_range.end.get::<millimeter>(),
            );
        }
        Ok(())
    }
    #[test]
    fn an_extruded_solid_places_its_faces_along_its_axis() -> OpmResult<()> {
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let extruded = Extruded::new(
            axis,
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Face::new(
                SurfaceShape::Plane,
                Isometry::new_along_z(millimeter!(20.0))?,
            ),
            circular_cross_section(millimeter!(12.5))?,
        )?;
        let frame = |index: FaceIndex| {
            extruded
                .face(index)
                .map(|face| *face.frame())
                .ok_or_else(|| OpossumError::Other(format!("there is no face {index}")))
        };
        assert_eq!(frame(Extruded::FRONT)?, axis);
        assert_eq!(
            frame(Extruded::REAR)?,
            axis.append(&Isometry::new_along_z(millimeter!(20.0))?)
        );
        Ok(())
    }
    #[test]
    fn an_extruded_body_is_placed_along_its_axis() -> OpmResult<()> {
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let mut body = Extruded::new(
            axis,
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Face::new(
                SurfaceShape::Plane,
                Isometry::new_along_z(millimeter!(20.0))?,
            ),
            circular_cross_section(millimeter!(12.5))?,
        )?;
        let node_frame = Isometry::new(millimeter!(0.0, 5.0, 0.0), degree!(0.0, 0.0, 0.0))?;
        body.move_by(&node_frame);
        assert_eq!(body.isometry(), node_frame.append(&axis));
        // halfway along the extrusion axis, which runs along the node's x axis
        let halfway = node_frame
            .append(&axis)
            .transform_point(&millimeter!(0.0, 0.0, 10.0));
        assert!(body.contains(&halfway));
        Ok(())
    }
    /// The chord and the box of an extruded body follow its axis, not the node: pinned for an axis
    /// that is turned and shifted against the node.
    #[test]
    fn an_extruded_body_measures_chords_and_its_box_along_its_axis() -> OpmResult<()> {
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let mut body = Extruded::new(
            axis,
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Face::new(
                SurfaceShape::Plane,
                Isometry::new_along_z(millimeter!(20.0))?,
            ),
            circular_cross_section(millimeter!(12.5))?,
        )?;
        let node_frame = Isometry::new(millimeter!(0.0, 5.0, 0.0), degree!(0.0, 0.0, 0.0))?;
        body.move_by(&node_frame);
        // The extrusion axis runs along the node's x axis, from x = 3 mm to x = 23 mm, 5 mm off the
        // node's origin in y. The circle of 12.5 mm radius lies around that line.
        let ray_at = |y: f64, z: f64| {
            Ray::new(
                millimeter!(-10.0, y, z),
                nalgebra::Vector3::x(),
                nanometer!(1053.0),
                joule!(1.0),
            )
        };
        let chord = body
            .path_length_inside(&ray_at(5.0, 0.0)?)
            .ok_or_else(|| OpossumError::Other("the ray along the axis missed the body".into()))?;
        assert_abs_diff_eq!(chord.get::<millimeter>(), 20.0, epsilon = 1e-9);
        // 11 mm off the axis, inside the circle around the body's axis but outside a circle around
        // the node's own origin, and the other way round at y = -10 mm: only a rim that is asked in
        // the node's frame tells them apart.
        let off_axis = body
            .path_length_inside(&ray_at(16.0, 0.0)?)
            .ok_or_else(|| OpossumError::Other("the ray 11 mm off the axis missed".into()))?;
        assert_abs_diff_eq!(off_axis.get::<millimeter>(), 20.0, epsilon = 1e-9);
        assert!(body.path_length_inside(&ray_at(-10.0, 0.0)?).is_none());
        assert!(body.path_length_inside(&ray_at(5.0, 13.0)?).is_none());
        // The box is stated in the body's own frame, where the extrusion runs along z.
        let bounds = body.bounding_box()?;
        for (range, from, to) in [
            (bounds.x_range(), -12.5, 12.5),
            (bounds.y_range(), -12.5, 12.5),
            (bounds.z_range(), 0.0, 20.0),
        ] {
            assert_abs_diff_eq!(range.start.get::<millimeter>(), from, epsilon = 1e-9);
            assert_abs_diff_eq!(range.end.get::<millimeter>(), to, epsilon = 1e-9);
        }
        Ok(())
    }
    #[test]
    fn path_length_leaving_sideways() -> OpmResult<()> {
        // a ray leaving through the barrel of the body is not modelled and thus reported as a miss
        let body = disk(millimeter!(10.0), millimeter!(5.0))?;
        let ray = test_ray(millimeter!(0.0, 0.0, 1.0), Vector3::new(0.0, 1.0, 0.1))?;
        assert_eq!(body.path_length_inside(&ray), None);
        Ok(())
    }
}
