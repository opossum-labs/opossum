//! Closed triangle meshes of bounded bodies.

use super::SurfaceBoundedBody;
use crate::{
    apertures::ApertureMesh,
    error::{OpmResult, OpossumError},
    geometry::SurfaceMesh,
};
use nalgebra::{Point2, Vector3};

/// A closed triangle mesh of a [`SurfaceBoundedBody`].
///
/// The three faces are kept apart rather than merged into one list of triangles, because the body
/// meets itself at a hard edge along its rim: a point there belongs to a bounding surface and to
/// the lateral face at once, and the two face in entirely different directions. Sharing it would
/// round that edge off.
///
/// All three are expressed in the body's own frame, so moving the body leaves them untouched.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyMesh {
    entrance: SurfaceMesh,
    exit: SurfaceMesh,
    edge: SurfaceMesh,
}

impl BodyMesh {
    /// The face light enters through, facing out of the body.
    #[must_use]
    pub const fn entrance(&self) -> &SurfaceMesh {
        &self.entrance
    }
    /// The face light leaves through, facing out of the body.
    #[must_use]
    pub const fn exit(&self) -> &SurfaceMesh {
        &self.exit
    }
    /// The lateral face closing the body between the other two, facing out of it.
    ///
    /// This is the rim of the clear aperture drawn out along the body: no light interacts with it,
    /// but without it the body would be two loose sheets rather than a solid.
    #[must_use]
    pub const fn edge(&self) -> &SurfaceMesh {
        &self.edge
    }
    /// All three faces of the body, so that whoever works on the whole solid does not have to know
    /// what it is made of.
    #[must_use]
    pub const fn faces(&self) -> [&SurfaceMesh; 3] {
        [&self.entrance, &self.exit, &self.edge]
    }
}

/// How sharply the rim has to turn at a point before that counts as a corner, as the cosine of the
/// angle between the two adjacent outward normals.
///
/// The rim of a circle is a polygon only because it was sampled into one, and drawing it with a
/// crease at every sample would show that. Averaging the two normals wherever the turn is gentle
/// makes it read as round again, while a real corner — of a rectangle, or of the dent in an L —
/// keeps both of its normals and stays sharp.
const SMOOTH_ENOUGH_TO_ROUND: f64 = 0.866; // cos(30°)

impl SurfaceBoundedBody {
    /// Build a closed triangle mesh of this body.
    ///
    /// The clear aperture is triangulated once and both bounding surfaces are laid over that same
    /// mesh, which is what makes their rims agree point for point; the lateral face is then drawn
    /// between those rims and closes the body.
    ///
    /// # Arguments
    ///
    /// - `segments`: the number of points to sample the rim of the clear aperture with
    ///
    /// # Returns
    ///
    /// The three faces of the body, all in the body's own frame and all facing outwards.
    ///
    /// # Errors
    ///
    /// This function returns an error if the clear aperture cannot be triangulated, if either
    /// surface cannot be laid over it (see [`GeoSurfaceRef::mesh_over`](crate::geometry::geo_surface::GeoSurfaceRef::mesh_over)),
    /// or if the two surfaces cross inside the aperture, so that there is no body between them.
    pub fn triangulate(&self, segments: usize) -> OpmResult<BodyMesh> {
        let aperture = self.cross_section.get().triangulate(segments)?;
        // The two surfaces are meshed one after the other and never held at once: a node with a
        // single surface hands out the same `GeoSurfaceRef` twice, which would deadlock.
        let entrance = self.entrance.mesh_over(&aperture, &self.isometry)?;
        let exit = self.exit.mesh_over(&aperture, &self.isometry)?;
        // Both surfaces were sampled above the very same positions, so whether they cross can be
        // read off point by point rather than guessed at from their rims.
        for (entrance_point, exit_point) in entrance.points().iter().zip(exit.points()) {
            if exit_point.z < entrance_point.z {
                return Err(OpossumError::Other(format!(
                    "the two surfaces of the body cross {:?} from its axis, so it encloses nothing \
                     there",
                    Point2::new(entrance_point.x, entrance_point.y)
                )));
            }
        }
        let edge = lateral_face(&aperture, &entrance, &exit)?;
        Ok(BodyMesh {
            // Light enters through this one, so the outside is the side it comes from.
            entrance: entrance.flipped(),
            exit,
            edge,
        })
    }
}

/// Draw the lateral face between the rims of the two bounding surfaces.
///
/// Both rims are the rim of the same [`ApertureMesh`], so the face is built by walking that rim once
/// and spanning two triangles between each pair of neighbouring positions. Its points are taken from
/// the two surface meshes verbatim, which is what makes the three faces meet without a seam.
///
/// # Arguments
///
/// - `aperture`: the triangulated clear aperture both surfaces were laid over
/// - `entrance`: the meshed entrance surface, still facing +z
/// - `exit`: the meshed exit surface
///
/// # Returns
///
/// The lateral face, facing away from the body's axis.
///
/// # Errors
///
/// This function returns an error if the face needs more points than can be indexed.
// The one cast below is guarded by the `try_from` on the very first line: no more than `4 * rim`
// points are ever pushed.
#[allow(clippy::cast_possible_truncation)]
fn lateral_face(
    aperture: &ApertureMesh,
    entrance: &SurfaceMesh,
    exit: &SurfaceMesh,
) -> OpmResult<SurfaceMesh> {
    let rim = aperture.rim_len();
    // Every rim position needs at most two pairs of points, and one index has to address them all.
    u32::try_from(4 * rim).map_err(|_| {
        OpossumError::Other("the lateral face has more points than can be indexed".into())
    })?;
    // The outward normal of the rim segment leaving each position. The rim runs counter-clockwise
    // seen from +z, so turning its direction a right angle clockwise points away from the body.
    // Every one of them is needed twice — once leaving a position, once arriving at the next.
    let segment_normals = (0..rim)
        .map(|step| {
            let (from, to) = (aperture.points()[step], aperture.points()[(step + 1) % rim]);
            Vector3::new((to.y - from.y).value, (from.x - to.x).value, 0.0).normalize()
        })
        .collect::<Vec<_>>();
    let mut points = Vec::with_capacity(4 * rim);
    let mut normals = Vec::with_capacity(4 * rim);
    // Where each rim segment begins and ends in the point list. A position the rim merely curves
    // through carries one pair of points with an averaged normal, so the face reads as round; one
    // it turns a corner at carries two, so the corner stays sharp.
    let mut begins = vec![0_u32; rim];
    let mut ends = vec![0_u32; rim];
    for step in 0..rim {
        let arriving = segment_normals[(step + rim - 1) % rim];
        let leaving = segment_normals[step];
        let mut stack_pair = |normal: Vector3<f64>| -> u32 {
            let index = points.len() as u32;
            // The entrance point first and the exit point right after it, so that one index
            // addresses both ends of the rim at this position.
            for surface in [entrance, exit] {
                points.push(surface.points()[step]);
                normals.push(normal);
            }
            index
        };
        if arriving.dot(&leaving) > SMOOTH_ENOUGH_TO_ROUND {
            let shared = stack_pair((arriving + leaving).normalize());
            begins[step] = shared;
            ends[step] = shared;
        } else {
            ends[step] = stack_pair(arriving);
            begins[step] = stack_pair(leaving);
        }
    }
    let mut triangles = Vec::with_capacity(2 * rim);
    for step in 0..rim {
        let (near, far) = (begins[step], ends[(step + 1) % rim]);
        // Seen from outside the rim runs counter-clockwise with the entrance below the exit, so
        // this is the winding that leaves both triangles facing outwards.
        triangles.push([near, far, near + 1]);
        triangles.push([far, far + 1, near + 1]);
    }
    Ok(SurfaceMesh::new(points, normals, triangles))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Aperture, ApertureType},
        degree,
        geometry::{
            Cylinder, Plane, Sphere,
            body::Body,
            geo_surface::{GeoSurface, GeoSurfaceRef},
        },
        meter, millimeter,
        types::validated_type_definitions::ValidatedCrossSection,
        utils::{
            geom_transformation::Isometry,
            test_helper::test_helper::{l_shape_corners, placed, somewhere_else},
        },
    };
    use nalgebra::Point3;
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };
    use uom::si::f64::Length;

    const SEGMENTS: usize = 32;
    /// How far to step off a face to decide which side of the body one lands on, in meter.
    const STEP: f64 = 1e-6;

    fn circle(radius: f64) -> OpmResult<ValidatedCrossSection> {
        ValidatedCrossSection::try_new(Aperture::new_circle(
            millimeter!(radius),
            ApertureType::Hole,
            None,
        )?)
    }

    fn rectangle(width: f64, height: f64) -> OpmResult<ValidatedCrossSection> {
        ValidatedCrossSection::try_new(Aperture::new_rectangle(
            millimeter!(width),
            millimeter!(height),
            ApertureType::Hole,
            None,
            None,
        )?)
    }

    fn l_shape() -> OpmResult<ValidatedCrossSection> {
        ValidatedCrossSection::try_new(Aperture::new_polygon(
            l_shape_corners(),
            ApertureType::Hole,
            None,
            None,
        )?)
    }

    /// One body of every shape the volume nodes build, placed in the given frame.
    fn volumes(placement: &Isometry) -> OpmResult<Vec<(&'static str, SurfaceBoundedBody)>> {
        let sphere = |radius: f64, z: f64| -> OpmResult<GeoSurfaceRef> {
            placed(
                Sphere::new(millimeter!(radius), Isometry::identity())?,
                placement,
                millimeter!(z),
            )
        };
        let cylinder = |radius: f64, z: f64| -> OpmResult<GeoSurfaceRef> {
            placed(
                Cylinder::new(millimeter!(radius), Isometry::identity())?,
                placement,
                millimeter!(z),
            )
        };
        let plane = |z: f64| -> OpmResult<GeoSurfaceRef> {
            placed(Plane::default(), placement, millimeter!(z))
        };
        let tilted_plane = |z: f64, angle: f64| -> OpmResult<GeoSurfaceRef> {
            let mut surface = Plane::default();
            surface.set_isometry(placement.append(&Isometry::new(
                millimeter!(0.0, 0.0, z),
                degree!(angle, 0.0, 0.0),
            )?));
            Ok(GeoSurfaceRef(Arc::new(Mutex::new(surface))))
        };
        Ok(vec![
            (
                "biconvex lens",
                SurfaceBoundedBody::new(
                    sphere(50.0, 0.0)?,
                    sphere(-50.0, 6.0)?,
                    circle(10.0)?,
                    *placement,
                ),
            ),
            (
                "meniscus lens",
                SurfaceBoundedBody::new(
                    sphere(50.0, 0.0)?,
                    sphere(80.0, 4.0)?,
                    circle(10.0)?,
                    *placement,
                ),
            ),
            (
                "plano-convex lens",
                SurfaceBoundedBody::new(
                    plane(0.0)?,
                    sphere(-50.0, 5.0)?,
                    circle(10.0)?,
                    *placement,
                ),
            ),
            (
                "cylindric lens",
                SurfaceBoundedBody::new(
                    cylinder(40.0, 0.0)?,
                    plane(5.0)?,
                    circle(10.0)?,
                    *placement,
                ),
            ),
            (
                "wedge",
                SurfaceBoundedBody::new(
                    plane(0.0)?,
                    tilted_plane(5.0, 8.0)?,
                    circle(8.0)?,
                    *placement,
                ),
            ),
            (
                "lens over an L",
                SurfaceBoundedBody::new(sphere(60.0, 0.0)?, plane(8.0)?, l_shape()?, *placement),
            ),
        ])
    }

    /// Quantize a point, so that a corner reached through two different faces hashes alike.
    // Picometres of a body measured in millimetres: nowhere near the range of an i64.
    #[allow(clippy::cast_possible_truncation)]
    fn vertex_key(point: &Point3<Length>) -> [i64; 3] {
        [point.x, point.y, point.z].map(|coordinate| (coordinate.value * 1e12).round() as i64)
    }

    /// Stepping off a face along its normal has to leave the body, and stepping the other way has
    /// to stay in it. That pins down both where the faces lie and which way they face.
    #[test]
    fn every_face_of_a_body_points_out_of_it() -> OpmResult<()> {
        for placement in [Isometry::identity(), somewhere_else()?] {
            for (name, body) in volumes(&placement)? {
                let mesh = body.triangulate(SEGMENTS)?;
                // The rim of a bounding surface is the outline of the cross section, asked for
                // directly rather than by triangulating it a second time.
                let rim = body.cross_section.get().outline_points(SEGMENTS)?.len();
                for (face, bounds_the_body) in [
                    (mesh.entrance(), true),
                    (mesh.exit(), true),
                    (mesh.edge(), false),
                ] {
                    for (index, (point, normal)) in
                        face.points().iter().zip(face.normals()).enumerate()
                    {
                        let from = body.isometry().transform_point(point);
                        let outwards = body.isometry().transform_vector_f64(normal);
                        let step = |sign: f64| {
                            Point3::new(
                                from.x + meter!(sign * STEP * outwards.x),
                                from.y + meter!(sign * STEP * outwards.y),
                                from.z + meter!(sign * STEP * outwards.z),
                            )
                        };
                        assert!(
                            !body.contains(&step(1.0))?,
                            "a point just outside the {name} was taken for being in it"
                        );
                        // Stepping the other way has to land inside — except from the rim of a
                        // bounding surface, which lies on the edge of the cross section too and
                        // would leave it sideways on the slightest rounding.
                        if bounds_the_body && index >= rim {
                            assert!(
                                body.contains(&step(-1.0))?,
                                "a point just inside the {name} was taken for being outside it"
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// A solid has no holes and no loose sheets: once the corners the three faces share are
    /// recognised as the same, every edge is shared by exactly two triangles.
    #[test]
    fn the_mesh_of_a_body_is_closed() -> OpmResult<()> {
        for placement in [Isometry::identity(), somewhere_else()?] {
            for (name, body) in volumes(&placement)? {
                let mesh = body.triangulate(SEGMENTS)?;
                let mut edges = HashMap::<[[i64; 3]; 2], usize>::new();
                for face in mesh.faces() {
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
    /// is rounded over it — while a real corner keeps both of its normals and stays sharp.
    #[test]
    fn the_lateral_face_rounds_curves_but_keeps_corners() -> OpmResult<()> {
        let placement = Isometry::identity();
        for (name, cross_section, corners) in [
            ("circle", circle(10.0)?, 0),
            ("rectangle", rectangle(20.0, 8.0)?, 4),
            ("L", l_shape()?, 6),
        ] {
            let rim = cross_section.get().outline_points(SEGMENTS)?.len();
            let body = SurfaceBoundedBody::new(
                placed(Plane::default(), &placement, millimeter!(0.0))?,
                placed(Plane::default(), &placement, millimeter!(5.0))?,
                cross_section,
                placement,
            );
            // One pair of points per rim position, and a second pair at every corner so that the
            // two sides meeting there keep their own normals.
            assert_eq!(
                body.triangulate(SEGMENTS)?.edge().points().len(),
                2 * (rim + corners),
                "the lateral face of the {name} was not creased where it should be"
            );
        }
        Ok(())
    }

    /// Two surfaces curving into each other steeply enough meet before the rim is reached, leaving
    /// no body out there at all — a lens ground thinner than its own sag.
    #[test]
    fn a_body_whose_surfaces_cross_cannot_be_meshed() -> OpmResult<()> {
        let placement = Isometry::identity();
        let too_thin = SurfaceBoundedBody::new(
            placed(
                Sphere::new(millimeter!(20.0), Isometry::identity())?,
                &placement,
                millimeter!(0.0),
            )?,
            placed(
                Sphere::new(millimeter!(-20.0), Isometry::identity())?,
                &placement,
                millimeter!(0.5),
            )?,
            circle(10.0)?,
            placement,
        );
        assert!(too_thin.triangulate(SEGMENTS).is_err());
        Ok(())
    }
}
