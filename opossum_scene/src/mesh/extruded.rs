//! Drawing a body extruded between two faces.

use nalgebra::Vector3;
use opossum_core::{
    apertures::{Bore, Outline},
    core_optics::optic_surface::OpticSurface,
    error::{OpmResult, OpossumError},
    geometry::{Extruded, FaceIndex, GeometryKind, SurfacePlacement, body::Body},
};
use std::iter::once;

use super::{
    Meshable,
    outline_mesh::OutlineMesh,
    surface_mesh::{FacePatch, SurfaceMesh},
};

/// How far a point of the rear face may sit below the front face in the mesh of an [`Extruded`]
/// solid before the two faces genuinely cross, rather than merely meeting.
///
/// The two faces are allowed to touch — a lens whose center thickness is exactly twice the sag of
/// two hemispherical faces meeting at the rim is a complete ball, not an error. But the two points
/// reach that shared value through independent chains of floating point operations, so where they
/// are mathematically equal they need not agree bit-for-bit. Scaled by the larger of the two
/// magnitudes rather than fixed, so the same relative slack applies whether the body is measured in
/// micrometres or metres.
const SURFACE_TOUCHING_TOLERANCE: f64 = 1e-9;

/// The profile is triangulated once and both faces are laid over that same mesh, which is what
/// makes their rims agree point for point; the lateral face is then drawn between those rims and
/// closes the body. All three face outwards.
///
/// # Errors
///
/// This function returns an error if the profile cannot be triangulated, if either face cannot be
/// laid over it (see [`SurfaceMesh::over`]), or if the two faces cross within the profile, so that
/// there is no body between them.
impl Meshable for Extruded {
    fn mesh(&self, segments: usize, _unbounded: &Outline) -> OpmResult<Vec<FacePatch>> {
        let face = |index: FaceIndex| {
            GeometryKind::surface(self, index)
                .map(OpticSurface::face)
                .ok_or_else(|| OpossumError::Other(format!("a body has no face {index}")))
        };
        let (front_face, rear_face) = (face(Self::FRONT)?, face(Self::REAR)?);
        let outline = self.rim().outline();
        let frame = Body::isometry(self);
        let front_at = SurfacePlacement::new(front_face, &frame)?;
        let rear_at = SurfacePlacement::new(rear_face, &frame)?;
        let in_plane = OutlineMesh::flat(outline, segments)?;
        // Both faces are asked above the very same positions, so whether they cross can be read
        // off point by point rather than guessed at from their rims.
        for position in in_plane.points() {
            let front_z = front_at.above(position)?.0.z.value;
            let rear_z = rear_at.above(position)?.0.z.value;
            let tolerance = SURFACE_TOUCHING_TOLERANCE * front_z.abs().max(rear_z.abs());
            if rear_z < front_z - tolerance {
                return Err(OpossumError::Other(format!(
                    "the two faces of the body cross {position:?} from its axis, so it encloses \
                     nothing there"
                )));
            }
        }
        // A tilted bore meets each face somewhere else, so each face gets its own rims.
        let front = SurfaceMesh::over(
            front_face,
            &OutlineMesh::on(outline, segments, |position| {
                Ok(front_at.above(&position)?.0.z)
            })?,
            &frame,
        )?;
        let rear = SurfaceMesh::over(
            rear_face,
            &OutlineMesh::on(outline, segments, |position| {
                Ok(rear_at.above(&position)?.0.z)
            })?,
            &frame,
        )?;
        // The edge runs along the axis of the body, a bore along its own.
        let axes: Vec<Vector3<f64>> = once(Vector3::z())
            .chain(outline.bore().map(Bore::axis))
            .collect();
        let edge = SurfaceMesh::between(&in_plane, &axes, &front, &rear)?;
        // Light enters through the front face, so the outside is the side it comes from.
        Ok([
            (Some(Self::FRONT), front.flipped()),
            (Some(Self::REAR), rear),
            (None, edge),
        ]
        .into_iter()
        .map(|(face, mesh)| FacePatch {
            face,
            mesh: mesh.transformed(&frame),
        })
        .collect())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        mesh::ring::RingPoints,
        test_helpers::{assert_meshed_as_the_core_does, l_shape_corners, somewhere_else},
    };
    use nalgebra::Point3;
    use opossum_core::{
        apertures::{CircleShape, OutlineShape, PolygonShape, RectangleShape},
        degree,
        geometry::{Cylinder, Face, Sphere, SurfaceShape},
        material::Material,
        meter, millimeter,
        utils::geom_transformation::Isometry,
    };
    use std::collections::HashMap;
    use uom::si::f64::{Angle, Length};

    const SEGMENTS: usize = 32;
    /// How far to step off a face to decide which side of the body one lands on, in meter.
    const STEP: f64 = 1e-6;

    /// Create the outline of a circular cross section.
    fn circular_cross_section(radius: Length) -> OpmResult<Outline> {
        Outline::new(CircleShape::new(radius)?, None)
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
        let mut body = Extruded::new(
            Isometry::identity(),
            front,
            rear,
            outline,
            Material::default(),
        )?;
        GeometryKind::move_by(&mut body, &node_frame);
        Ok(body)
    }
    /// Mesh the given body; a solid is bounded all around, so the unbounded outline goes unused.
    fn patches_of(body: &Extruded) -> OpmResult<Vec<FacePatch>> {
        Meshable::mesh(body, SEGMENTS, body.rim().outline())
    }
    /// The meshes of the given body's patches, without the faces they lie on.
    fn mesh_of(body: &Extruded) -> OpmResult<Vec<SurfaceMesh>> {
        Ok(patches_of(body)?
            .into_iter()
            .map(|patch| patch.mesh)
            .collect())
    }
    /// A body is drawn as its front face, its rear face and the lateral face between them, which
    /// is no face of its own.
    #[test]
    fn every_patch_of_a_body_lies_on_its_face() -> OpmResult<()> {
        for (name, body) in volumes(&Isometry::identity())? {
            let faces: Vec<_> = patches_of(&body)?
                .into_iter()
                .map(|patch| patch.face)
                .collect();
            assert_eq!(
                faces,
                [Some(Extruded::FRONT), Some(Extruded::REAR), None],
                "the patches of the {name} lie on the wrong faces"
            );
        }
        Ok(())
    }
    /// How many points of the mesh of an outline lie on its rims; they lead the points.
    fn rim_points(outline: &Outline) -> OpmResult<usize> {
        let mesh = OutlineMesh::flat(outline, SEGMENTS)?;
        Ok(mesh.rims().last().map_or(0, |rim| rim.end))
    }
    fn rectangle(width: f64, height: f64) -> OpmResult<RectangleShape> {
        RectangleShape::new(millimeter!(width), millimeter!(height))
    }
    /// The given edge with a bore through it, beside its center.
    fn bored(edge: impl Into<OutlineShape>, bore: impl Into<OutlineShape>) -> OpmResult<Outline> {
        Outline::new(
            edge,
            Some(Bore::new(bore, millimeter!(2.0, -1.0), degree!(0.0))),
        )
    }
    /// A circle of 10 mm with a round bore of 3 mm beside its center, tilted by the given angles.
    fn tilted_bore(about_x: Angle, about_y: Angle) -> OpmResult<Outline> {
        Outline::new(
            CircleShape::new(millimeter!(10.0))?,
            Some(
                Bore::new(
                    CircleShape::new(millimeter!(3.0))?,
                    millimeter!(2.0, -1.0),
                    degree!(0.0),
                )
                .with_tilt(about_x, about_y),
            ),
        )
    }
    /// One body of every shape the volume nodes build, at the given node frame.
    fn volumes(node_frame: &Isometry) -> OpmResult<Vec<(&'static str, Extruded)>> {
        let sphere = |radius: f64, z: f64| sphere_face(millimeter!(radius), millimeter!(z));
        let plane = |z: f64| -> OpmResult<Face> {
            Ok(Face::new(
                SurfaceShape::Plane,
                Isometry::new_along_z(millimeter!(z))?,
            ))
        };
        let circle = |radius: f64| circular_cross_section(millimeter!(radius));
        let l_shape = Outline::new(PolygonShape::new(l_shape_corners())?, None)?;
        let shapes = vec![
            (
                "biconvex lens",
                sphere(50.0, 0.0)?,
                sphere(-50.0, 6.0)?,
                circle(10.0)?,
            ),
            (
                "meniscus lens",
                sphere(50.0, 0.0)?,
                sphere(80.0, 4.0)?,
                circle(10.0)?,
            ),
            (
                "plano-convex lens",
                plane(0.0)?,
                sphere(-50.0, 5.0)?,
                circle(10.0)?,
            ),
            (
                "cylindric lens",
                Face::new(
                    SurfaceShape::Cylinder(Cylinder::new(millimeter!(40.0))?),
                    Isometry::identity(),
                ),
                plane(5.0)?,
                circle(10.0)?,
            ),
            (
                "wedge",
                plane(0.0)?,
                Face::new(
                    SurfaceShape::Plane,
                    Isometry::new(millimeter!(0.0, 0.0, 5.0), degree!(8.0, 0.0, 0.0))?,
                ),
                circle(8.0)?,
            ),
            ("lens over an L", sphere(60.0, 0.0)?, plane(8.0)?, l_shape),
            (
                "plate with a bore",
                plane(0.0)?,
                plane(5.0)?,
                bored(
                    CircleShape::new(millimeter!(10.0))?,
                    CircleShape::new(millimeter!(3.0))?,
                )?,
            ),
            (
                "lens with a square bore",
                sphere(50.0, 0.0)?,
                sphere(-50.0, 6.0)?,
                bored(CircleShape::new(millimeter!(10.0))?, rectangle(3.0, 3.0)?)?,
            ),
            (
                "plate with a tilted bore",
                plane(0.0)?,
                plane(5.0)?,
                tilted_bore(degree!(0.0), degree!(30.0))?,
            ),
            (
                "lens with a tilted bore",
                sphere(50.0, 0.0)?,
                sphere(-50.0, 6.0)?,
                tilted_bore(degree!(15.0), degree!(-10.0))?,
            ),
        ];
        shapes
            .into_iter()
            .map(|(name, front, rear, outline)| {
                Ok((name, body_between(front, rear, outline, *node_frame)?))
            })
            .collect()
    }
    /// Quantize a point, so that a corner reached through two different faces hashes alike.
    // Picometers of a body measured in millimeters: nowhere near the range of an i64.
    #[allow(clippy::cast_possible_truncation)]
    fn vertex_key(point: &Point3<Length>) -> [i64; 3] {
        [point.x, point.y, point.z].map(|coordinate| (coordinate.value * 1e12).round() as i64)
    }
    /// Stepping off a face along its normal has to leave the body, and stepping the other way has
    /// to stay in it. That pins down both where the faces lie and which way they face.
    ///
    /// The step starts a hair inside a triangle rather than on its very corner. Where two walls
    /// meet in a corner pointing into the body, as in a square bore, a step from the corner along
    /// the normal of one wall runs along the other, and rounding would decide which side it is on.
    #[test]
    fn every_face_of_a_body_points_out_of_it() -> OpmResult<()> {
        /// How far towards the center of its triangle a step starts, as a fraction of the way.
        const INTO_THE_TRIANGLE: f64 = 1e-3;
        for node_frame in [Isometry::identity(), somewhere_else()?] {
            for (name, body) in volumes(&node_frame)? {
                let rim = rim_points(body.rim().outline())?;
                let meshes = mesh_of(&body)?;
                // front, rear, then the lateral face, which bounds nothing a step could land in
                for (index, face) in meshes.iter().enumerate() {
                    let bounds_the_body = index < 2;
                    for triangle in face.triangles() {
                        let corners = triangle.map(|index| face.points()[index as usize]);
                        let to_the_center = |corner: Point3<Length>| {
                            ((corners[0] - corner) + (corners[1] - corner) + (corners[2] - corner))
                                .map(|c| c / 3.)
                        };
                        for (index, corner) in
                            triangle.map(|index| index as usize).iter().zip(corners)
                        {
                            let from =
                                corner + to_the_center(corner).map(|c| c * INTO_THE_TRIANGLE);
                            let outwards = face.normals()[*index];
                            let step = |sign: f64| {
                                Point3::new(
                                    from.x + meter!(sign * STEP * outwards.x),
                                    from.y + meter!(sign * STEP * outwards.y),
                                    from.z + meter!(sign * STEP * outwards.z),
                                )
                            };
                            assert!(
                                !body.contains(&step(1.0)),
                                "a point just outside the {name} was taken for being in it"
                            );
                            // Stepping the other way has to land inside - except from the rim of
                            // a bounding face, which lies on the edge of the profile too and would
                            // leave it sideways on the slightest rounding.
                            if bounds_the_body && *index >= rim {
                                assert!(
                                    body.contains(&step(-1.0)),
                                    "a point just inside the {name} was taken for being outside it"
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    /// A solid has no holes and no loose sheets: once the corners the three faces share are
    /// recognized as the same, every edge is shared by exactly two triangles.
    #[test]
    fn the_mesh_of_a_body_is_closed() -> OpmResult<()> {
        for node_frame in [Isometry::identity(), somewhere_else()?] {
            for (name, body) in volumes(&node_frame)? {
                let meshes = mesh_of(&body)?;
                assert_eq!(meshes.len(), 3, "the {name} is not drawn as three faces");
                let mut edges = HashMap::<[[i64; 3]; 2], usize>::new();
                for face in &meshes {
                    for triangle in face.triangles() {
                        let corners =
                            triangle.map(|index| vertex_key(&face.points()[index as usize]));
                        for corner in 0..3 {
                            let mut edge = [corners[corner], corners[(corner + 1) % 3]];
                            edge.sort_unstable();
                            *edges.entry(edge).or_default() += 1;
                        }
                    }
                }
                assert!(!edges.is_empty(), "the {name} was not meshed at all");
                for (edge, shared_by) in &edges {
                    assert_eq!(
                        *shared_by, 2,
                        "an edge of the {name} belongs to {shared_by} triangles instead of two: \
                         {edge:?}"
                    );
                }
            }
        }
        Ok(())
    }
    /// The rim of a circle is a polygon only because it was sampled into one, so the lateral face
    /// is rounded over it - while a real corner keeps both of its normals and stays sharp.
    #[test]
    fn the_lateral_face_rounds_curves_but_keeps_corners() -> OpmResult<()> {
        let circle = || CircleShape::new(millimeter!(10.0));
        let l_shape = || PolygonShape::new(l_shape_corners());
        for (name, outline, corners) in [
            ("circle", Outline::new(circle()?, None)?, 0),
            ("rectangle", Outline::new(rectangle(20.0, 8.0)?, None)?, 4),
            ("L", Outline::new(l_shape()?, None)?, 6),
            (
                "circle with a bore",
                bored(circle()?, CircleShape::new(millimeter!(3.0))?)?,
                0,
            ),
            (
                "rectangle with a square bore",
                bored(rectangle(20.0, 8.0)?, rectangle(3.0, 3.0)?)?,
                8,
            ),
        ] {
            let rim = rim_points(&outline)?;
            let (front, rear) = plate_faces(millimeter!(5.0))?;
            let body = body_between(front, rear, outline, Isometry::identity())?;
            // One pair of points per rim position, and a second pair at every corner so that the
            // two sides meeting there keep their own normals.
            assert_eq!(
                mesh_of(&body)?[2].points().len(),
                2 * (rim + corners),
                "the lateral face of the {name} was not creased where it should be"
            );
        }
        Ok(())
    }
    /// A lens ground thinner than its own sag: its two faces curve into each other and meet before
    /// the rim is reached.
    fn too_thin() -> OpmResult<Extruded> {
        body_between(
            sphere_face(millimeter!(20.0), millimeter!(0.0))?,
            sphere_face(millimeter!(-20.0), millimeter!(0.5))?,
            circular_cross_section(millimeter!(10.0))?,
            Isometry::identity(),
        )
    }
    /// Two faces curving into each other steeply enough meet before the rim is reached, leaving no
    /// body out there at all.
    #[test]
    fn a_body_whose_faces_cross_cannot_be_meshed() -> OpmResult<()> {
        let error = mesh_of(&too_thin()?).unwrap_err();
        assert!(error.to_string().contains("cross"), "{error}");
        Ok(())
    }
    /// The tightest a body can be without its faces crossing: two hemispheres meeting exactly at
    /// the rim, center thickness equal to twice the sag (25 mm each). Geometrically this is a
    /// complete ball, not an error, so it pins down [`SURFACE_TOUCHING_TOLERANCE`] - mirrored at
    /// the node level by `mesh::test::a_hemisphere_lens_can_be_meshed`.
    #[test]
    fn two_hemispheres_meeting_exactly_at_the_rim_are_not_a_crossing() -> OpmResult<()> {
        let ball = body_between(
            sphere_face(millimeter!(25.0), millimeter!(0.0))?,
            sphere_face(millimeter!(-25.0), millimeter!(50.0))?,
            circular_cross_section(millimeter!(25.0))?,
            Isometry::identity(),
        )?;
        mesh_of(&ball)?;
        Ok(())
    }
    /// Transitional: every body is meshed bit for bit as the core does, wherever its node sits,
    /// and one whose faces cross is refused alike.
    #[test]
    fn a_body_is_meshed_as_the_core_does() -> OpmResult<()> {
        for node_frame in [Isometry::identity(), somewhere_else()?] {
            for (name, body) in volumes(&node_frame)? {
                assert_meshed_as_the_core_does(
                    name,
                    GeometryKind::mesh(&body, SEGMENTS, body.rim().outline()),
                    patches_of(&body),
                );
            }
        }
        let too_thin = too_thin()?;
        assert_meshed_as_the_core_does(
            "lens too thin for its sag",
            GeometryKind::mesh(&too_thin, SEGMENTS, too_thin.rim().outline()),
            patches_of(&too_thin),
        );
        // The rims the scene samples are the ones the core samples.
        for (name, body) in volumes(&Isometry::identity())? {
            let shape = body.rim().outline().shape();
            assert_eq!(
                shape.ring_points(SEGMENTS)?,
                shape.outline_points(SEGMENTS)?,
                "the rim of the {name}"
            );
        }
        Ok(())
    }
}
