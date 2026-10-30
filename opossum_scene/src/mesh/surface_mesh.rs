//! Triangle meshes of faces, and of the lateral face closing a body between two of them.

use nalgebra::{Point3, Vector3};
use opossum_core::{
    error::{OpmResult, OpossumError},
    geometry::{Face, FaceIndex, SurfacePlacement},
    utils::geom_transformation::Isometry,
};
use std::ops::Range;
use uom::si::f64::Length;

use super::outline_mesh::OutlineMesh;

/// One meshed patch of a geometry, together with the face it lies on.
///
/// Knowing the face is what lets each face of a component be drawn in a material of its own. The
/// lateral face closing a body between two faces is no face of the geometry and has none.
#[derive(Debug, Clone, PartialEq)]
pub struct FacePatch {
    /// The face the patch lies on, or `None` for the lateral face of a body.
    pub face: Option<FaceIndex>,
    /// The meshed patch.
    pub mesh: SurfaceMesh,
}

/// A triangle mesh of one piece of surface, with a normal at every point.
///
/// Usually that piece is a [`Face`] laid over an [`OutlineMesh`] (see [`SurfaceMesh::over`]); the
/// lateral face closing a body between two of them is the one exception (see
/// [`SurfaceMesh::between`]).
///
/// The mesh lives in the frame of the geometry it was built from — that of an optical node — and
/// not in global coordinates. Moving the node therefore leaves it untouched: only the node's own
/// placement has to be passed on again, which is what lets a changed setup be redrawn without
/// recomputing any geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceMesh {
    points: Vec<Point3<Length>>,
    normals: Vec<Vector3<f64>>,
    triangles: Vec<[u32; 3]>,
}

/// How sharply the rim has to turn at a point before that counts as a corner, as the cosine of the
/// angle between the two adjacent outward normals.
///
/// The rim of a circle is a polygon only because it was sampled into one, and drawing it with a
/// crease at every sample would show that. Averaging the two normals wherever the turn is gentle
/// makes it read as round again, while a real corner — of a rectangle, or of the dent in an L —
/// keeps both of its normals and stays sharp.
const SMOOTH_ENOUGH_TO_ROUND: f64 = 0.866; // cos(30°)

impl SurfaceMesh {
    /// Lay a flat [`OutlineMesh`] onto a face.
    ///
    /// The outline mesh states *where* to sample — a triangulation of the area the face is wanted
    /// over — and the face answers where it lies above each of those positions and which way it
    /// faces there (see [`SurfacePlacement::above`]). Splitting it this way is what lets a lens
    /// mesh both of its faces over one outline.
    ///
    /// Points and triangles come out in the order the outline mesh had them, so
    /// [`OutlineMesh::rims`] are the rims of the result too.
    ///
    /// # Arguments
    ///
    /// - `face`: the face to lay the outline mesh onto
    /// - `outline_mesh`: the triangulated area to lay onto the face, in the xy plane of `frame`
    /// - `frame`: the frame to express the result in, given in the frame the face is stated in
    ///
    /// # Returns
    ///
    /// The meshed face, with all normals facing the +z of `frame`.
    ///
    /// # Errors
    ///
    /// This function returns an error if the face runs parallel to the frame's axis, if it does
    /// not reach as far out as the outline, or if it is both curved and tilted against the frame.
    pub fn over(face: &Face, outline_mesh: &OutlineMesh, frame: &Isometry) -> OpmResult<Self> {
        let placement = SurfacePlacement::new(face, frame)?;
        let mut points = Vec::with_capacity(outline_mesh.points().len());
        let mut normals = Vec::with_capacity(outline_mesh.points().len());
        for transversal_position in outline_mesh.points() {
            let (point, normal) = placement.above(transversal_position)?;
            points.push(point);
            normals.push(normal);
        }
        Ok(Self {
            points,
            normals,
            triangles: outline_mesh.triangles().to_vec(),
        })
    }
    /// Draw the lateral face closing a body between two faces meshed over the rims of one
    /// outline.
    ///
    /// Both faces have the same rims, point for point, so the lateral face is built by walking
    /// each rim once and spanning two triangles between each pair of neighboring positions. Its
    /// points are taken from the two face meshes verbatim, which is what makes the faces meet
    /// without a seam. Each rim runs through the body along its own direction, such as a tilted
    /// bore along its axis, which the lateral face there is parallel to.
    ///
    /// # Arguments
    ///
    /// - `outline_mesh`: the outline in the plane both faces were laid over, which states the rims
    ///   and where they cross the plane
    /// - `axes`: the direction each rim runs along through the body, one per rim: the edge along
    ///   the axis of the component, a bore along its own
    /// - `lower`: the meshed face bounding the body towards -z, still facing +z
    /// - `upper`: the meshed face bounding the body towards +z
    ///
    /// # Returns
    ///
    /// The lateral face, facing away from the material of the body.
    ///
    /// # Errors
    ///
    /// This function returns an error if there is not one direction per rim, or if the face needs
    /// more points than can be indexed.
    // The one cast below is guarded by the `try_from` on the very first line: no more than
    // `4 * rim_points` points are ever pushed.
    #[allow(clippy::cast_possible_truncation)]
    pub fn between(
        outline_mesh: &OutlineMesh,
        axes: &[Vector3<f64>],
        lower: &Self,
        upper: &Self,
    ) -> OpmResult<Self> {
        if axes.len() != outline_mesh.rims().len() {
            return Err(OpossumError::Other(format!(
                "the lateral face needs one direction for each of its {} rims, not {}",
                outline_mesh.rims().len(),
                axes.len()
            )));
        }
        let rim_points = outline_mesh.rims().iter().map(Range::len).sum::<usize>();
        // Every rim position needs at most two pairs of points, and one index has to address them
        // all.
        u32::try_from(4 * rim_points).map_err(|_| {
            OpossumError::Other("the lateral face has more points than can be indexed".into())
        })?;
        let mut points = Vec::with_capacity(4 * rim_points);
        let mut normals = Vec::with_capacity(4 * rim_points);
        let mut triangles = Vec::with_capacity(2 * rim_points);
        for (rim, axis) in outline_mesh.rims().iter().zip(axes) {
            let len = rim.len();
            // The outward normal of the rim segment leaving each position. A rim runs with the
            // body on its left seen from +z, so the segment crossed with the direction the rim runs
            // along points away from the body. Every one is needed twice — once leaving a position,
            // once arriving at the next.
            let segment_normals = (0..len)
                .map(|step| {
                    let from = outline_mesh.points()[rim.start + step];
                    let to = outline_mesh.points()[rim.start + (step + 1) % len];
                    Vector3::new((to.x - from.x).value, (to.y - from.y).value, 0.0)
                        .cross(axis)
                        .normalize()
                })
                .collect::<Vec<_>>();
            // Where each rim segment begins and ends in the point list. A position the rim merely
            // curves through carries one pair of points with an averaged normal, so the face reads
            // as round; one it turns a corner at carries two, so the corner stays sharp.
            let mut begins = vec![0_u32; len];
            let mut ends = vec![0_u32; len];
            for step in 0..len {
                let arriving = segment_normals[(step + len - 1) % len];
                let leaving = segment_normals[step];
                let mut stack_pair = |normal: Vector3<f64>| -> u32 {
                    let index = points.len() as u32;
                    // The lower point first and the upper point right after it, so that one index
                    // addresses both ends of the rim at this position.
                    for face in [lower, upper] {
                        points.push(face.points()[rim.start + step]);
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
            for step in 0..len {
                let (near, far) = (begins[step], ends[(step + 1) % len]);
                // Seen from outside, the rim runs counter-clockwise with the lower face below the
                // upper one, so this is the winding that leaves both triangles facing outwards.
                triangles.push([near, far, near + 1]);
                triangles.push([far, far + 1, near + 1]);
            }
        }
        Ok(Self {
            points,
            normals,
            triangles,
        })
    }
    /// The points of the mesh, in the frame it was built in.
    ///
    /// Point `i` lies above point `i` of the [`OutlineMesh`] the face was meshed over, so the
    /// outline's [`rims`](OutlineMesh::rims) are the rims of this mesh as well.
    #[must_use]
    pub fn points(&self) -> &[Point3<Length>] {
        &self.points
    }
    /// The unit normal at every point, in the same order and the same frame.
    #[must_use]
    pub fn normals(&self) -> &[Vector3<f64>] {
        &self.normals
    }
    /// The triangles of the mesh as index triples into [`points`](SurfaceMesh::points).
    ///
    /// Every triple is ordered counter-clockwise when seen from the side the normals point to.
    #[must_use]
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
    /// Express the mesh in another frame.
    ///
    /// # Arguments
    ///
    /// - `iso`: where the frame the mesh was built in sits within the frame to express it in
    ///
    /// # Returns
    ///
    /// The same mesh, its points and normals carried through `iso`.
    #[must_use]
    pub fn transformed(mut self, iso: &Isometry) -> Self {
        for point in &mut self.points {
            *point = iso.transform_point(point);
        }
        for normal in &mut self.normals {
            *normal = iso.transform_vector_f64(normal);
        }
        self
    }
    /// Turn the mesh around, so that it faces the other way.
    ///
    /// A body is bounded from below by one face and from above by another, and what is outside is
    /// on opposite sides of the two. Since a face is always meshed facing +z, one of them has to be
    /// turned over. Both the normals and the winding of the triangles are reversed, so the two keep
    /// agreeing on which side is out.
    #[must_use]
    pub fn flipped(mut self) -> Self {
        for normal in &mut self.normals {
            *normal = -*normal;
        }
        for triangle in &mut self.triangles {
            triangle.swap(1, 2);
        }
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_helpers::somewhere_else;
    use approx::assert_abs_diff_eq;
    use nalgebra::Point2;
    use opossum_core::{
        apertures::{CircleShape, Outline},
        degree,
        geometry::{Cylinder, Parabola, Sphere, SurfaceShape},
        millimeter,
    };

    #[test]
    fn flipping_turns_normals_and_winding_together() -> OpmResult<()> {
        let outline = Outline::new(CircleShape::new(millimeter!(10.0))?, None)?;
        let face = Face::new(
            SurfaceShape::Sphere(Sphere::new(millimeter!(50.0))?),
            Isometry::identity(),
        );
        let mesh = SurfaceMesh::over(
            &face,
            &OutlineMesh::flat(&outline, 24)?,
            &Isometry::identity(),
        )?;
        let flipped = mesh.clone().flipped();
        assert_eq!(flipped.points(), mesh.points());
        for (before, after) in mesh.normals().iter().zip(flipped.normals()) {
            assert_eq!(*after, -before);
        }
        for (before, after) in mesh.triangles().iter().zip(flipped.triangles()) {
            assert_eq!(*after, [before[0], before[2], before[1]]);
        }
        assert_eq!(
            flipped.flipped(),
            mesh,
            "turning it over twice turns it back"
        );
        Ok(())
    }

    /// The number of points the rim of an outline is sampled with in the mesh tests.
    const MESH_SEGMENTS: usize = 24;

    /// A circle of the given radius in millimeter, triangulated.
    fn circle_mesh(radius: f64) -> OpmResult<OutlineMesh> {
        OutlineMesh::flat(
            &Outline::new(CircleShape::new(millimeter!(radius))?, None)?,
            MESH_SEGMENTS,
        )
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
    /// Check everything [`SurfaceMesh::over`] promises.
    fn check_mesh_contract(
        name: &str,
        face: &Face,
        frame: &Isometry,
        outline_mesh: &OutlineMesh,
    ) -> OpmResult<()> {
        let mesh = SurfaceMesh::over(face, outline_mesh, frame)?;
        assert_eq!(mesh.points().len(), outline_mesh.points().len());
        assert_eq!(mesh.normals().len(), outline_mesh.points().len());
        assert_eq!(mesh.triangles(), outline_mesh.triangles());
        // Where the face itself says it lies above each meshed point - asked through its own
        // frame rather than the one the mesh was built from, so this is an independent answer.
        for point in mesh.points() {
            let local = face
                .frame()
                .inverse_transform_point(&frame.transform_point(point));
            let sag = face
                .surface()
                .local_z_at(&Point2::new(local.x, local.y))
                .expect("the face reaches a point it was meshed at");
            assert_abs_diff_eq!(local.z.value, sag.value, epsilon = 1e-12);
        }
        for normal in mesh.normals() {
            assert_abs_diff_eq!(normal.norm(), 1.0, epsilon = 1e-12);
            assert!(
                normal.z > 0.0,
                "a normal of the {name} does not face the +z of its frame: {normal:?}"
            );
        }
        for triangle in mesh.triangles() {
            let corner =
                |index: u32| mesh.points()[usize::try_from(index).unwrap()].map(|c| c.value);
            let [first, second, third] = triangle.map(corner);
            let normal = |index: u32| mesh.normals()[usize::try_from(index).unwrap()];
            let facing = (second - first).cross(&(third - first));
            let mean_normal =
                (normal(triangle[0]) + normal(triangle[1]) + normal(triangle[2])) / 3.;
            assert!(
                facing.dot(&mean_normal) > 0.0,
                "a triangle of the {name} is wound against the normals of its corners"
            );
        }
        Ok(())
    }
    #[test]
    fn a_meshed_face_follows_the_contract() -> OpmResult<()> {
        let outline_mesh = circle_mesh(10.0)?;
        for node in [Isometry::identity(), somewhere_else()?] {
            for (name, face) in faces_of_a_node(&node)? {
                check_mesh_contract(name, &face, &node, &outline_mesh)?;
            }
        }
        Ok(())
    }
    /// The mesh is built in the node's own frame, so moving the node leaves it alone. That is what
    /// lets a changed setup be redrawn by resending placements instead of geometry.
    #[test]
    fn the_mesh_does_not_move_with_the_node() -> OpmResult<()> {
        let outline_mesh = circle_mesh(10.0)?;
        let (here, there) = (Isometry::identity(), somewhere_else()?);
        for ((name, face), (_, moved)) in faces_of_a_node(&here)?
            .into_iter()
            .zip(faces_of_a_node(&there)?)
        {
            let at_origin = SurfaceMesh::over(&face, &outline_mesh, &here)?;
            let elsewhere = SurfaceMesh::over(&moved, &outline_mesh, &there)?;
            assert_eq!(at_origin.triangles(), elsewhere.triangles());
            for (before, after) in at_origin.points().iter().zip(elsewhere.points()) {
                for axis in 0..3 {
                    assert_abs_diff_eq!(before[axis].value, after[axis].value, epsilon = 1e-12);
                }
            }
            for (before, after) in at_origin.normals().iter().zip(elsewhere.normals()) {
                assert_abs_diff_eq!((before - after).norm(), 0.0, epsilon = 1e-12);
            }
            assert!(
                !at_origin.points().is_empty(),
                "the {name} was not meshed at all"
            );
        }
        Ok(())
    }
    /// A sphere ends where its radius of curvature does. Asked beyond that it has no answer, and a
    /// mesh reaching further out must not be filled in with something made up.
    #[test]
    fn a_face_too_small_for_the_outline_cannot_be_meshed() -> OpmResult<()> {
        let face = Face::new(
            SurfaceShape::Sphere(Sphere::new(millimeter!(5.0))?),
            Isometry::identity(),
        );
        assert!(SurfaceMesh::over(&face, &circle_mesh(10.0)?, &Isometry::identity()).is_err());
        assert!(SurfaceMesh::over(&face, &circle_mesh(2.0)?, &Isometry::identity()).is_ok());
        Ok(())
    }
    /// A face standing on edge never crosses the frame's axis, so there is no point "above" any
    /// transversal position to mesh.
    #[test]
    fn a_face_parallel_to_the_frame_axis_cannot_be_meshed() -> OpmResult<()> {
        let on_edge = Face::new(
            SurfaceShape::Plane,
            Isometry::new(millimeter!(0.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?,
        );
        assert!(SurfaceMesh::over(&on_edge, &circle_mesh(10.0)?, &Isometry::identity()).is_err());
        Ok(())
    }
    /// Where a curved face tilted against its frame is met cannot be worked out by walking along
    /// the frame's axis, so it is turned away - the same limit the extent of a body is subject to.
    #[test]
    fn a_curved_face_tilted_against_the_frame_cannot_be_meshed() -> OpmResult<()> {
        let node = Isometry::identity();
        for shape in [
            SurfaceShape::Sphere(Sphere::new(millimeter!(50.0))?),
            SurfaceShape::Cylinder(Cylinder::new(millimeter!(40.0))?),
            SurfaceShape::Parabola(Parabola::new(millimeter!(60.0))?),
        ] {
            let face = tilted(shape, &node)?;
            assert!(SurfaceMesh::over(&face, &circle_mesh(10.0)?, &node).is_err());
        }
        Ok(())
    }
}
