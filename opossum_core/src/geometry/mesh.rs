//! Triangle meshes of faces, and of the lateral face closing a body between two of them.

use crate::{
    apertures::OutlineMesh,
    error::{OpmResult, OpossumError},
    geometry::FaceIndex,
    utils::geom_transformation::Isometry,
};
use nalgebra::{Point3, Vector3};
use std::ops::Range;
use uom::si::f64::Length;

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
/// Usually that piece is a [`Face`](super::Face) laid over an [`OutlineMesh`] (see
/// [`GeometryKind::mesh`](super::GeometryKind::mesh)); the lateral face closing a body between two
/// of them is the one exception (see [`SurfaceMesh::between`]).
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
    /// Assemble a mesh from points, the normal at each of them, and the triangles between them.
    pub(super) const fn new(
        points: Vec<Point3<Length>>,
        normals: Vec<Vector3<f64>>,
        triangles: Vec<[u32; 3]>,
    ) -> Self {
        Self {
            points,
            normals,
            triangles,
        }
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
    /// - `axes`: the direction each rim runs along through the body, one per rim (see
    ///   [`Outline::rim_axes`](crate::apertures::Outline::rim_axes))
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
    pub(super) fn between(
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
        Ok(Self::new(points, normals, triangles))
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
    pub(crate) fn transformed(mut self, iso: &Isometry) -> Self {
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
    use crate::{
        apertures::{CircleShape, Outline},
        geometry::{Face, Sphere, SurfaceShape},
        millimeter,
    };

    #[test]
    fn flipping_turns_normals_and_winding_together() -> OpmResult<()> {
        let outline = Outline::new(CircleShape::new(millimeter!(10.0))?, None)?;
        let face = Face::new(
            SurfaceShape::Sphere(Sphere::new(millimeter!(50.0))?),
            Isometry::identity(),
        );
        let mesh = face.mesh_over(&outline.triangulate(24)?, &Isometry::identity())?;
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
}
