#![warn(missing_docs)]
//! One optical surface of a component: its shape and the frame of its vertex.
//!
//! A [`Face`] is a [`SurfaceShape`] at a frame. Stated relative to the component it belongs to, it
//! can be compared and stored; moved to where the component sits (see [`Face::move_by`]), it is what
//! rays are intersected with. The math of the shape is done in the vertex frame (see
//! [`GeoSurface`]); the face turns rays and points into that frame and back.

use std::fmt::{self, Display, Formatter};

use nalgebra::{Point2, Point3, Vector3};
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use crate::{
    error::{OpmResult, OpossumError},
    geometry::{GeoSurface, SurfaceShape},
    light::Ray,
    nanometer, radian,
    utils::geom_transformation::Isometry,
};

/// The number of a face in its geometry: its position among the faces the geometry has (see
/// [`GeometryKind`](crate::geometry::GeometryKind)).
///
/// An extruded solid has its front face as 0 and its rear face as 1, a surface has the one face 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceIndex(usize);

impl FaceIndex {
    /// Create the number of the face at the given position in its geometry.
    #[must_use]
    pub const fn new(position: usize) -> Self {
        Self(position)
    }
}

impl From<FaceIndex> for usize {
    fn from(index: FaceIndex) -> Self {
        index.0
    }
}

impl Display for FaceIndex {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One optical surface of a component: its [`SurfaceShape`] and the frame of its vertex.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Face {
    shape: SurfaceShape,
    frame: Isometry,
}

impl Face {
    /// Create a new [`Face`].
    ///
    /// # Arguments
    ///
    /// * `shape` - the shape of the surface.
    /// * `frame` - position and orientation of the vertex, in the frame this face is stated in.
    #[must_use]
    pub const fn new(shape: SurfaceShape, frame: Isometry) -> Self {
        Self { shape, frame }
    }
    /// Returns the shape of this [`Face`].
    #[must_use]
    pub const fn shape(&self) -> &SurfaceShape {
        &self.shape
    }
    /// Returns the frame of the vertex of this [`Face`].
    #[must_use]
    pub const fn frame(&self) -> &Isometry {
        &self.frame
    }
    /// Returns the surface of this [`Face`], to be asked in the vertex frame.
    #[must_use]
    pub fn surface(&self) -> &dyn GeoSurface {
        self.shape.as_geo_surface()
    }
    /// Move this face by an isometry: its vertex frame becomes `by` followed by its own frame.
    ///
    /// # Arguments
    ///
    /// * `by` - the frame this face is stated in, given in the frame it is moved to.
    pub fn move_by(&mut self, by: &Isometry) {
        self.frame = by.append(&self.frame);
    }
    /// Calculate the intersection point of a [`Ray`] with the unbounded surface of this face and
    /// the normal there.
    ///
    /// # Arguments
    ///
    /// * `ray` - the ray, in the frame this face is stated in.
    ///
    /// # Returns
    ///
    /// The intersection point and the normalized surface normal there, pointing against the ray
    /// direction, both in the frame this face is stated in; or `None` if the ray misses the
    /// surface.
    #[must_use]
    pub fn intersect(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        let local_ray = ray.inverse_transformed_ray(&self.frame);
        self.surface()
            .calc_intersect_and_normal(&local_ray)
            .map(|(point, normal)| {
                (
                    self.frame.transform_point(&point),
                    self.frame.transform_vector_f64(&normal),
                )
            })
    }
    /// Return whether a point lies behind the surface of this face (see [`GeoSurface::is_behind`]).
    ///
    /// # Arguments
    ///
    /// * `point` - the point, in the frame this face is stated in.
    #[must_use]
    pub fn is_behind(&self, point: &Point3<Length>) -> bool {
        self.surface()
            .is_behind(&self.frame.inverse_transform_point(point))
    }
    /// Return whether a point of the surface of this face lies on the sheet through its vertex.
    ///
    /// A [`Sphere`](crate::geometry::Sphere) or [`Cylinder`](crate::geometry::Cylinder) closes on
    /// itself, so a ray can meet it on its far side, behind the equator, which is not part of a
    /// component built from it. [`GeoSurface::local_z_at`] always answers for the sheet through
    /// the vertex, so a point lies on that sheet if its own local z agrees. A single-sheeted
    /// surface always does.
    ///
    /// # Arguments
    ///
    /// * `point` - a point on the surface, in the frame this face is stated in.
    ///
    /// # Returns
    ///
    /// `true` if the point lies on the sheet through the vertex, up to floating-point noise.
    #[must_use]
    pub fn is_on_vertex_sheet(&self, point: &Point3<Length>) -> bool {
        let local_point = self.frame.inverse_transform_point(point);
        self.surface()
            .local_z_at(&Point2::new(local_point.x, local_point.y))
            .is_some_and(|sheet_z| (local_point.z - sheet_z).abs() <= nanometer!(1.0))
    }
    /// Check that this face reaches as far out as the outline that bounds it.
    ///
    /// The vertex is taken to sit on the axis of the outline, as it does for every face the
    /// presets build.
    ///
    /// # Arguments
    ///
    /// * `transversal_reach` - how far the outline reaches from its axis.
    /// * `name` - how to name this face in the error, e.g. `"front face"`.
    ///
    /// # Errors
    ///
    /// This function returns an error if the face ends before the outline does.
    pub(crate) fn check_reach(&self, transversal_reach: Length, name: &str) -> OpmResult<()> {
        if self.surface().reaches(transversal_reach) {
            return Ok(());
        }
        Err(OpossumError::Other(format!(
            "the {name} does not reach the edge of its optic shape, {:.3} mm from the axis: its \
             radius of curvature is smaller",
            transversal_reach.get::<uom::si::length::millimeter>()
        )))
    }
    /// Whether this face is the same surface at the same place as another face.
    ///
    /// Both faces have to have the same shape, and their vertices have to sit at the same point with
    /// their axes pointing the same way. How far a face is turned about its axis matters only for a
    /// shape that is not symmetric about it, a cylinder. Deviations as small as floating-point noise
    /// are tolerated.
    ///
    /// # Arguments
    ///
    /// * `other` - the face to compare with, stated in the same frame as this one.
    ///
    /// # Returns
    ///
    /// `true` if both faces describe the same surface.
    #[must_use]
    pub fn coincides_with(&self, other: &Self) -> bool {
        // Tolerances that only absorb floating-point noise, as for the axes of two node inputs.
        let angle_tolerance = radian!(1.0e-9);
        let length_tolerance = nanometer!(1.0);
        // With both axes aligned, the local y axes have to run along the same line; half a turn
        // reverses them and still maps a cylinder onto itself.
        let same_roll = || {
            self.frame
                .transform_vector_f64(&Vector3::y())
                .cross(&other.frame.transform_vector_f64(&Vector3::y()))
                .norm()
                <= angle_tolerance.value
        };
        self.shape.matches(&other.shape, length_tolerance)
            && self
                .frame
                .axis_mismatch(&other.frame, angle_tolerance, length_tolerance)
                .is_none()
            && (!self.shape.depends_on_roll() || same_roll())
    }
}

/// How far a face may be out of square with a frame before that counts as a tilt, given as the
/// sine of the angle between the two axes.
///
/// Composing a frame with its own inverse does not come out exactly, so a face placed square in
/// its node still reports a tilt of a few floating point epsilons — which would otherwise turn
/// every curved face in a node that is merely rotated into an unsupported one. The threshold sits
/// far above that noise and far below any angle a component is built with: a wedge is turned by
/// degrees, not by billionths of one. The same slack decides when a face is so nearly parallel to
/// the frame's axis that it no longer bounds anything along it.
const ALIGNMENT_TOLERANCE: f64 = 1e-9;

/// How a [`Face`] stands in a frame that is not its own.
///
/// Everything built *from* faces — the extent of a body, a mesh laid over one — is expressed in
/// the frame of the component instead, which need not agree with the frame of the face: a wedge
/// tilts its exit face against the body it bounds. This does that translation once, together with
/// the checks that decide whether the face can bound that frame at all.
pub struct SurfacePlacement<'a> {
    surface: &'a dyn GeoSurface,
    relative: Isometry,
    axis: Vector3<f64>,
    tilt: f64,
    vertex: Length,
}

impl<'a> SurfacePlacement<'a> {
    /// Work out how `face` stands in `frame`.
    ///
    /// # Arguments
    ///
    /// - `face`: the face to place
    /// - `frame`: the frame to measure it against, given in the frame the face is stated in
    ///
    /// # Errors
    ///
    /// This function returns an error if the face runs parallel to the frame's axis, so that it
    /// never crosses it and cannot bound anything along it, or if the face has no vertex.
    pub fn new(face: &'a Face, frame: &Isometry) -> OpmResult<Self> {
        let surface = face.surface();
        let relative =
            Isometry::new_from_transform(frame.get_inv_transform() * face.frame.get_transform());
        // The frame's own z axis, written in the face's frame: dotted with a point of the face it
        // yields that point's longitudinal position in the frame. Its transversal part states how
        // far the face is tilted against the frame, its z part how much of the face's own sag
        // survives into the frame's z.
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
    /// Maps a point of the face's own frame into the frame it was measured against.
    pub(crate) const fn relative(&self) -> &Isometry {
        &self.relative
    }
    /// How far the face is tilted against the frame's axis, as the sine of that angle.
    pub(crate) const fn tilt(&self) -> f64 {
        self.tilt
    }
    /// Whether the face is turned against the frame's axis at all, rather than square to it.
    ///
    /// See [`ALIGNMENT_TOLERANCE`] for why this is not simply `tilt() > 0.0`.
    pub(crate) fn is_tilted(&self) -> bool {
        self.tilt > ALIGNMENT_TOLERANCE
    }
    /// How much of the face's own sag survives into the frame's z, as the cosine of the tilt.
    ///
    /// Negative if the face sits the other way round in this frame.
    pub(crate) fn alignment(&self) -> f64 {
        self.axis.z
    }
    /// The longitudinal position of the vertex in the face's own frame.
    pub(crate) const fn vertex(&self) -> Length {
        self.vertex
    }
    /// The surface of the face, to be asked in its vertex frame.
    pub(crate) fn surface(&self) -> &dyn GeoSurface {
        self.surface
    }
    /// The point of the face above a transversal position of the frame, and the way it faces
    /// there — both in the frame's coordinates.
    ///
    /// "Above" is along the frame's axis: the point where a line through the given transversal
    /// position, parallel to that axis, meets the face. Finding it means walking along that line
    /// until the face's own transversal plane is reached, which is exact in the two cases that
    /// occur: for a face parallel to the frame the walk does not move transversally at all, so any
    /// sag above the position found is the answer; for a flat one there is no sag to miss. A face
    /// both tilted *and* curved would be met somewhere the walk does not reach, and is turned away
    /// — the same limit the extent of a body is subject to, and no node builds such a face.
    ///
    /// The normal is turned towards the frame's +z, so it is the outward normal of a body this face
    /// bounds from below and the inward one of a body it bounds from above.
    ///
    /// # Arguments
    ///
    /// - `transversal_position`: the position in the frame's xy plane to look above
    ///
    /// # Returns
    ///
    /// The point of the face and its unit normal, both in the frame's coordinates.
    ///
    /// # Errors
    ///
    /// This function returns an error if the face does not reach that far out, or if it is both
    /// curved and tilted against the frame.
    pub fn above(
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
            // The face may sit the other way round in this frame, in which case its own +z is the
            // frame's -z and the normal has to be turned around to keep the promise above.
            if self.alignment().is_sign_negative() {
                -normal
            } else {
                normal
            },
        ))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        degree,
        geometry::{Cylinder, Parabola, Sphere},
        millimeter,
        utils::test_helper::helper::somewhere_else,
    };
    use approx::assert_abs_diff_eq;

    /// A face moved to a node sits at the node followed by its own frame, and meets a ray there:
    /// a ray along the moved axis hits the vertex.
    #[test]
    fn a_moved_face_sits_at_the_node_followed_by_its_frame() -> OpmResult<()> {
        let node_frame = Isometry::new(millimeter!(1.0, 2.0, 3.0), degree!(10.0, 0.0, 5.0))?;
        let face = Face::new(
            SurfaceShape::Cylinder(Cylinder::new(millimeter!(50.0))?),
            Isometry::new_along_z(millimeter!(2.0))?,
        );
        let mut moved = face.clone();
        moved.move_by(&node_frame);
        assert_eq!(*moved.frame(), node_frame.append(face.frame()));
        let ray = Ray::new(
            node_frame.transform_point(&millimeter!(0.0, 0.0, -10.0)),
            node_frame.transform_vector_f64(&Vector3::z()),
            nanometer!(1053.0),
            crate::joule!(1.0),
        )?;
        let (point, _) = moved
            .intersect(&ray)
            .ok_or_else(|| OpossumError::Other("the ray missed the face".into()))?;
        let vertex = moved.frame().transform_point(&Point3::origin());
        assert_abs_diff_eq!(
            (point - vertex).map(|c| c.value).norm(),
            0.0,
            epsilon = 1e-15
        );
        Ok(())
    }
    #[test]
    fn a_face_coincides_only_with_the_same_surface_at_the_same_place() -> OpmResult<()> {
        let sphere = |radius: f64, vertex: Isometry| -> OpmResult<Face> {
            Ok(Face::new(
                SurfaceShape::Sphere(Sphere::new(millimeter!(radius))?),
                vertex,
            ))
        };
        let at = |z: f64| Isometry::new_along_z(millimeter!(z));
        let face = sphere(50.0, at(5.0)?)?;
        // the same surface moved there from another frame, or differing by rounding noise only
        let mut shifted = sphere(50.0, at(2.0)?)?;
        shifted.move_by(&at(3.0)?);
        assert!(face.coincides_with(&shifted));
        assert!(face.coincides_with(&sphere(50.0 + 1.0e-10, at(5.0)?)?));
        // another radius, another place, the axis flipped, another shape
        for other in [
            sphere(50.001, at(5.0)?)?,
            sphere(50.0, at(5.001)?)?,
            sphere(
                50.0,
                Isometry::new(millimeter!(0.0, 0.0, 5.0), degree!(180.0, 0.0, 0.0))?,
            )?,
            Face::new(
                SurfaceShape::Cylinder(Cylinder::new(millimeter!(50.0))?),
                at(5.0)?,
            ),
        ] {
            assert!(!face.coincides_with(&other), "{other:?}");
        }
        // a parabola is compared by its focal length
        let parabola = |focal_length: f64| -> OpmResult<Face> {
            Ok(Face::new(
                SurfaceShape::Parabola(Parabola::new(millimeter!(focal_length))?),
                Isometry::identity(),
            ))
        };
        assert!(parabola(100.0)?.coincides_with(&parabola(100.0)?));
        assert!(!parabola(100.0)?.coincides_with(&parabola(101.0)?));
        Ok(())
    }

    /// A face of the given shape the given distance along the axis of a node.
    fn along(shape: SurfaceShape, node: &Isometry, z: f64) -> OpmResult<Face> {
        Ok(Face::new(
            shape,
            node.append(&Isometry::new_along_z(millimeter!(z))?),
        ))
    }
    /// A face turned against its node, the way a wedge turns its exit face.
    fn tilted(shape: SurfaceShape, node: &Isometry) -> OpmResult<Face> {
        Ok(Face::new(
            shape,
            node.append(&Isometry::new(
                millimeter!(0.0, 0.0, 3.0),
                degree!(0.0, 8.0, 0.0),
            )?),
        ))
    }
    /// One of every face a volume node builds, each at its own offset within the node.
    fn faces_of_a_node(node: &Isometry) -> OpmResult<Vec<(&'static str, Face)>> {
        Ok(vec![
            ("plane", along(SurfaceShape::Plane, node, 0.0)?),
            (
                "convex sphere",
                along(SurfaceShape::spherical(millimeter!(50.0))?, node, 0.0)?,
            ),
            (
                "concave sphere",
                along(SurfaceShape::spherical(millimeter!(-50.0))?, node, 4.0)?,
            ),
            (
                "cylinder",
                along(SurfaceShape::cylindrical(millimeter!(40.0))?, node, 2.0)?,
            ),
            (
                "parabola",
                along(
                    SurfaceShape::Parabola(Parabola::new(millimeter!(60.0))?),
                    node,
                    0.0,
                )?,
            ),
            ("tilted plane", tilted(SurfaceShape::Plane, node)?),
        ])
    }

    /// Transversal positions of a frame to place points above, all within the reach of every face
    /// of [`faces_of_a_node`].
    fn positions() -> [Point2<Length>; 5] {
        [
            millimeter!(0.0, 0.0),
            millimeter!(6.0, 0.0),
            millimeter!(0.0, -7.0),
            millimeter!(3.0, 4.0),
            millimeter!(-6.0, -6.0),
        ]
    }
    /// A face placed in a frame answers with the point right above the asked position, lying on the
    /// face, and with a unit normal facing the +z of the frame.
    #[test]
    fn a_placed_face_follows_the_contract() -> OpmResult<()> {
        for node in [Isometry::identity(), somewhere_else()?] {
            for (name, face) in faces_of_a_node(&node)? {
                let placement = SurfacePlacement::new(&face, &node)?;
                for position in positions() {
                    let (point, normal) = placement.above(&position)?;
                    assert_abs_diff_eq!(point.x.value, position.x.value, epsilon = 1e-12);
                    assert_abs_diff_eq!(point.y.value, position.y.value, epsilon = 1e-12);
                    // Where the face itself says it lies there, asked through its own frame.
                    let local = face
                        .frame()
                        .inverse_transform_point(&node.transform_point(&point));
                    let sag = face
                        .surface()
                        .local_z_at(&Point2::new(local.x, local.y))
                        .expect("the face reaches a point it was placed at");
                    assert_abs_diff_eq!(local.z.value, sag.value, epsilon = 1e-12);
                    assert_abs_diff_eq!(normal.norm(), 1.0, epsilon = 1e-12);
                    assert!(
                        normal.z > 0.0,
                        "a normal of the {name} does not face the +z of its frame: {normal:?}"
                    );
                }
            }
        }
        Ok(())
    }
    /// A face is placed in its node's own frame, so moving the node changes no answer.
    #[test]
    fn a_placement_does_not_move_with_the_node() -> OpmResult<()> {
        let (here, there) = (Isometry::identity(), somewhere_else()?);
        for ((_, face), (_, moved)) in faces_of_a_node(&here)?
            .into_iter()
            .zip(faces_of_a_node(&there)?)
        {
            let at_origin = SurfacePlacement::new(&face, &here)?;
            let elsewhere = SurfacePlacement::new(&moved, &there)?;
            for position in positions() {
                let (before, before_normal) = at_origin.above(&position)?;
                let (after, after_normal) = elsewhere.above(&position)?;
                assert_abs_diff_eq!(
                    (before - after).map(|c| c.value).norm(),
                    0.0,
                    epsilon = 1e-12
                );
                assert_abs_diff_eq!((before_normal - after_normal).norm(), 0.0, epsilon = 1e-12);
            }
        }
        Ok(())
    }
    /// A sphere ends where its radius of curvature does, so it places no point beyond that.
    #[test]
    fn a_face_places_no_point_beyond_its_reach() -> OpmResult<()> {
        let face = Face::new(
            SurfaceShape::Sphere(Sphere::new(millimeter!(5.0))?),
            Isometry::identity(),
        );
        let placement = SurfacePlacement::new(&face, &Isometry::identity())?;
        assert!(placement.above(&millimeter!(10.0, 0.0)).is_err());
        assert!(placement.above(&millimeter!(2.0, 0.0)).is_ok());
        Ok(())
    }
    /// A face standing on edge never crosses the frame's axis, so it cannot be placed in that frame.
    #[test]
    fn a_face_parallel_to_the_frame_axis_cannot_be_placed() -> OpmResult<()> {
        let on_edge = Face::new(
            SurfaceShape::Plane,
            Isometry::new(millimeter!(0.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?,
        );
        assert!(SurfacePlacement::new(&on_edge, &Isometry::identity()).is_err());
        Ok(())
    }
    /// Where a curved face tilted against its frame is met cannot be found by walking along the
    /// frame's axis, so no point off its vertex is placed.
    #[test]
    fn a_curved_face_tilted_against_the_frame_places_no_point() -> OpmResult<()> {
        let node = Isometry::identity();
        for shape in [
            SurfaceShape::Sphere(Sphere::new(millimeter!(50.0))?),
            SurfaceShape::Cylinder(Cylinder::new(millimeter!(40.0))?),
            SurfaceShape::Parabola(Parabola::new(millimeter!(60.0))?),
        ] {
            let face = tilted(shape, &node)?;
            let placement = SurfacePlacement::new(&face, &node)?;
            assert!(placement.above(&millimeter!(5.0, 3.0)).is_err());
        }
        Ok(())
    }
}
