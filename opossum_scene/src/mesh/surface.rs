//! Drawing a single surface.

use opossum_core::{
    apertures::Outline,
    error::{OpmResult, OpossumError},
    geometry::{
        GeometryKind, SurfaceExtent, SurfaceGeometry, SurfacePlacement, body::default_optic_shape,
    },
};

use super::{
    Meshable,
    outline_mesh::OutlineMesh,
    surface_mesh::{FacePatch, SurfaceMesh},
};

/// Return the outline a face without an edge of its own is drawn over: the default optic shape,
/// around the face's vertex. A face is endless, but a drawing is not.
///
/// # Errors
///
/// This function returns an error if the default optic shape bounds no region.
pub(super) fn unbounded_outline() -> OpmResult<Outline> {
    default_optic_shape()
        .outline()
        .cloned()
        .ok_or_else(|| OpossumError::Other("the default optic shape bounds no region".into()))
}

/// The face is laid over the outline of its rim, measured along the axis of the rim, with a bore
/// where it meets the face; an open surface has no rim and is drawn over the
/// [`unbounded_outline`] around its own vertex.
impl Meshable for SurfaceGeometry {
    fn mesh(&self, segments: usize) -> OpmResult<Vec<FacePatch>> {
        let face = GeometryKind::surface(self, Self::FACE)
            .ok_or_else(|| OpossumError::Other("a surface has no face".into()))?
            .face();
        let unbounded;
        let (outline, frame) = match self.extent() {
            SurfaceExtent::Edge(rim) | SurfaceExtent::Window(rim) => (rim.outline(), *rim.frame()),
            SurfaceExtent::Open => {
                unbounded = unbounded_outline()?;
                (&unbounded, *face.frame())
            }
        };
        let at = SurfacePlacement::new(face, &frame)?;
        let mesh = SurfaceMesh::over(
            &at,
            &OutlineMesh::on(outline, segments, |position| Ok(at.above(&position)?.0.z))?,
        )?;
        Ok(vec![FacePatch {
            face: Some(Self::FACE),
            mesh: mesh.transformed(&frame),
        }])
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::assert_abs_diff_eq;
    use nalgebra::{Point2, Vector3};
    use opossum_core::{
        apertures::CircleShape,
        degree,
        geometry::{Geometry, Parabola, SurfaceShape},
        millimeter,
        utils::geom_transformation::Isometry,
    };
    use uom::si::length::millimeter;

    /// The number of points the rim of an outline is sampled with in the mesh tests.
    const MESH_SEGMENTS: usize = 24;

    fn circle() -> OpmResult<Outline> {
        Outline::new(CircleShape::new(millimeter!(12.5))?, None)
    }
    /// The one surface of a geometry.
    fn surface_of(geometry: &Geometry) -> OpmResult<&SurfaceGeometry> {
        match geometry {
            Geometry::Surface(surface) => Ok(surface),
            _ => Err(OpossumError::Other("the geometry is no surface".into())),
        }
    }
    /// Check that a surface was drawn as one patch on its face, reaching `reach` millimeter around
    /// the z axis of `frame` and no further, and facing along the +z of its face.
    fn check_drawn(
        geometry: &Geometry,
        meshes: &[FacePatch],
        frame: &Isometry,
        reach: f64,
    ) -> OpmResult<()> {
        let [FacePatch { face, mesh }] = meshes else {
            return Err(OpossumError::Other(format!(
                "a surface was drawn as {} patches",
                meshes.len()
            )));
        };
        assert_eq!(
            *face,
            Some(SurfaceGeometry::FACE),
            "the patch of a surface lies on its one face"
        );
        let face = geometry
            .face(SurfaceGeometry::FACE)
            .ok_or_else(|| OpossumError::Other("the surface has no face".into()))?;
        let mut farthest = 0.0_f64;
        for point in mesh.points() {
            let around_axis = frame.inverse_transform_point(point);
            let off_axis = around_axis
                .x
                .get::<millimeter>()
                .hypot(around_axis.y.get::<millimeter>());
            assert!(
                off_axis <= reach + 1e-9,
                "a point lies {off_axis} mm off axis"
            );
            farthest = farthest.max(off_axis);
            let on_face = face.frame().inverse_transform_point(point);
            let sag = face
                .surface()
                .local_z_at(&Point2::new(on_face.x, on_face.y))
                .ok_or_else(|| OpossumError::Other("the face misses a drawn point".into()))?;
            assert_abs_diff_eq!(on_face.z.value, sag.value, epsilon = 1e-12);
        }
        assert_abs_diff_eq!(farthest, reach, epsilon = 1e-9);
        let up = face.frame().transform_vector_f64(&Vector3::z());
        assert!(mesh.normals().iter().all(|normal| normal.dot(&up) > 0.0));
        Ok(())
    }
    /// An off-axis mirror cut around the axis of its rim, 50 mm beside its vertex, and a detector
    /// whose window sits there too.
    fn mirror_and_detector() -> OpmResult<(Isometry, [Geometry; 2])> {
        let axis = Isometry::new_translation(millimeter!(50.0, 0.0, 0.0))?;
        let beside = Isometry::new_translation(millimeter!(-50.0, 0.0, 0.0))?;
        let parabola = SurfaceShape::Parabola(Parabola::new(millimeter!(100.0))?);
        let mirror = Geometry::surface(axis, parabola, beside, Some(circle()?))?;
        let mut detector = Geometry::detector(SurfaceShape::Plane, Some(circle()?))?;
        detector.move_by(&axis);
        Ok((axis, [mirror, detector]))
    }
    /// A plane standing across the axis of its node, 20 mm along its own, without an edge.
    fn open_plane() -> OpmResult<(Isometry, Geometry)> {
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let vertex = Isometry::new_along_z(millimeter!(20.0))?;
        let geometry = Geometry::surface(axis, SurfaceShape::Plane, vertex, None)?;
        Ok((axis.append(&vertex), geometry))
    }
    /// An off-axis mirror is cut from its parent surface around the axis of its rim, not around
    /// its vertex: here a parabola whose vertex lies 50 mm beside the rim. A detector draws its
    /// window the same way.
    #[test]
    fn a_bounded_surface_is_drawn_over_its_rim_around_the_axis_of_the_rim() -> OpmResult<()> {
        let (axis, geometries) = mirror_and_detector()?;
        for geometry in geometries {
            let meshes = Meshable::mesh(surface_of(&geometry)?, MESH_SEGMENTS)?;
            check_drawn(&geometry, &meshes, &axis, 12.5)?;
        }
        Ok(())
    }
    /// An unbounded surface has no edge to end the drawing at, so it is drawn over the default
    /// optic shape of 12.5 mm, around its own vertex - even where that stands across the axis of
    /// the node.
    #[test]
    fn an_unbounded_surface_is_drawn_over_the_default_optic_shape_around_its_vertex()
    -> OpmResult<()> {
        let (vertex, geometry) = open_plane()?;
        let meshes = Meshable::mesh(surface_of(&geometry)?, MESH_SEGMENTS)?;
        check_drawn(&geometry, &meshes, &vertex, 12.5)
    }
}
