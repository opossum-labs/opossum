//! Turning the geometry of a component into triangles: its outlines sampled and filled, its faces
//! laid over them.
//!
//! The core states the geometry; every kind of it is drawn here by an implementation of
//! [`Meshable`] of its own.

mod assembly;
mod extruded;
mod outline_mesh;
mod parametric;
mod ring;
mod surface;
mod surface_mesh;

pub use surface_mesh::{FacePatch, SurfaceMesh};

use opossum_core::{apertures::Outline, error::OpmResult, geometry::Geometry};

/// A kind of geometry that can be drawn as triangle meshes.
pub trait Meshable {
    /// Return triangle meshes of the surfaces that bound this geometry, to draw it.
    ///
    /// Each mesh is one patch of surface: a face laid over the outline it ends at, or the lateral
    /// face closing a body between two faces. Every patch carries the number of the face it lies
    /// on; the lateral face has none. Patches do not share points, so the hard edge where two of
    /// them meet stays hard. Every patch faces out of a body; a single surface faces along the +z
    /// of its face.
    ///
    /// # Arguments
    ///
    /// * `segments` - the number of points to sample the rim of an outline with.
    /// * `unbounded` - the outline to draw a face over that has no edge of its own, such as an
    ///   unbounded surface; a face is endless, but a drawing is not.
    ///
    /// # Returns
    ///
    /// The meshed patches, in the frame the geometry is stated in.
    ///
    /// # Errors
    ///
    /// This function returns an error if an outline cannot be triangulated, if a face cannot be
    /// laid over it or if this kind of geometry cannot be meshed yet.
    fn mesh(&self, segments: usize, unbounded: &Outline) -> OpmResult<Vec<FacePatch>>;
}

/// Every kind of geometry is drawn by its own implementation. A new kind in the core stops this
/// from compiling until it can be drawn.
impl Meshable for Geometry {
    fn mesh(&self, segments: usize, unbounded: &Outline) -> OpmResult<Vec<FacePatch>> {
        match self {
            Self::Surface(surface) => surface.mesh(segments, unbounded),
            Self::Extruded(body) => body.mesh(segments, unbounded),
            Self::Parametric(solid) => solid.mesh(segments, unbounded),
            Self::Assembly(assembly) => assembly.mesh(segments, unbounded),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{appearance::Appearance, components::unbounded_outline};
    use approx::{assert_abs_diff_eq, assert_relative_eq};
    use nalgebra::Point2;
    use opossum_core::{
        analyzers::Analyzable,
        apertures::{Bore, CircleShape, OpticShape, OutlineShape},
        core_optics::{NodeAttrExt, OpticNode, node_attr::NodePositioning},
        degree,
        error::OpossumError,
        geometry::{Extruded, FaceIndex, SurfaceGeometry, body::OPTIC_SHAPE},
        millimeter,
        nodes::{
            BoredMirror, Dummy, Lens, ParabolicMirror, ThinMirror, create_node_ref, node_types,
        },
        properties::Proptype,
        refractive_index::RefrIndexConst,
        utils::geom_transformation::Isometry,
    };
    use std::f64::consts::PI;

    /// The patches a node is drawn as, through the scene's own mesher.
    fn node_patches(node: &dyn Analyzable, segments: usize) -> OpmResult<Vec<FacePatch>> {
        let geometry = node.create_geometry()?.ok_or_else(|| {
            OpossumError::Other(format!("node '{}' has no geometry to draw", node.name()))
        })?;
        geometry.mesh(segments, &unbounded_outline()?)
    }
    /// A node is meshed from its geometry: the unbounded plane of a dummy is drawn as far as the
    /// default optic shape reaches.
    #[test]
    fn a_node_is_meshed_from_its_geometry() -> OpmResult<()> {
        let meshes = node_patches(&Dummy::default(), 16)?;
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].face, Some(SurfaceGeometry::FACE));
        let farthest = meshes[0]
            .mesh
            .points()
            .iter()
            .map(|point| point.x.value.hypot(point.y.value))
            .fold(0.0, f64::max);
        let OutlineShape::Circle(circle) = unbounded_outline()?.shape().clone() else {
            return Err(OpossumError::Other(
                "the default optic shape is no circle".into(),
            ));
        };
        assert_abs_diff_eq!(farthest, circle.radius().value, epsilon = 1e-12);
        Ok(())
    }
    /// The one patch a surface node is drawn as.
    fn surface_mesh(node: &dyn Analyzable, segments: usize) -> OpmResult<SurfaceMesh> {
        let mut meshes = node_patches(node, segments)?;
        assert_eq!(meshes.len(), 1, "a surface is drawn as one patch");
        meshes
            .pop()
            .map(|patch| patch.mesh)
            .ok_or_else(|| OpossumError::Other("the surface was not meshed".into()))
    }
    /// A node in its default state — a flat mirror over the default optic shape — can be meshed at
    /// all, and the mesh actually covers ground rather than coming back empty.
    #[test]
    fn a_surface_node_can_be_meshed_in_its_default_state() -> OpmResult<()> {
        let mut mirror = ThinMirror::default();
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let mesh = surface_mesh(&mirror, 32)?;
        assert!(
            !mesh.triangles().is_empty(),
            "a mirror's default surface came out without any triangles"
        );
        assert_eq!(
            mesh.points().len(),
            mesh.normals().len(),
            "every meshed point needs a normal to shade or hit-test it"
        );
        Ok(())
    }
    /// The mesh is drawn from the node's properties as they are now, exactly like a volume node's
    /// body: a curvature set without rebuilding the surfaces already bends it. A sphere of radius R
    /// rises by R - sqrt(R² - r²) at the 12.5 mm rim of the default optic shape.
    #[test]
    fn the_mesh_follows_a_changed_property_without_a_rebuild() -> OpmResult<()> {
        let mut mirror = ThinMirror::default();
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let radius = 100.0_f64;
        mirror.set_property("curvature", Proptype::Curvature(millimeter!(radius)))?;
        let mesh = surface_mesh(&mirror, 32)?;
        let rise = mesh
            .points()
            .iter()
            .map(|point| point.z.value.abs())
            .fold(0.0, f64::max);
        assert_abs_diff_eq!(
            rise,
            millimeter!(radius - radius.mul_add(radius, -12.5 * 12.5).sqrt()).value,
            epsilon = 1e-9
        );
        Ok(())
    }
    /// The mesh lives in the node's own frame, exactly like a volume node's - moving the node must
    /// not bake a translation into the points themselves.
    #[test]
    fn the_mesh_does_not_depend_on_where_the_node_is_placed() -> OpmResult<()> {
        let at_origin = {
            let mut mirror = ThinMirror::default();
            mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
            surface_mesh(&mirror, 16)?
        };
        let far_away = {
            let mut mirror = ThinMirror::default();
            mirror.set_positioning(NodePositioning::Absolute(Isometry::new(
                millimeter!(0.0, 0.0, 500.0),
                degree!(0.0, 0.0, 0.0),
            )?))?;
            surface_mesh(&mirror, 16)?
        };
        assert_eq!(
            at_origin.points(),
            far_away.points(),
            "the surface mesh must be independent of the node's placement"
        );
        Ok(())
    }
    /// A bored mirror with a round bore of 4 mm through the center of its 12.5 mm disc, tilted by
    /// the given angles.
    fn bored_mirror(about_x: f64, about_y: f64) -> OpmResult<BoredMirror> {
        let ring = OpticShape::Bounded(Outline::new(
            CircleShape::new(millimeter!(12.5))?,
            Some(
                Bore::new(
                    CircleShape::new(millimeter!(4.0))?,
                    millimeter!(0., 0.),
                    degree!(0.),
                )
                .with_tilt(degree!(about_x), degree!(about_y)),
            ),
        )?);
        let mut mirror = BoredMirror::default();
        mirror.set_property(OPTIC_SHAPE, ring.into())?;
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        Ok(mirror)
    }
    /// A bored mirror is meshed around its bore: what is drawn of its mirrored face covers the
    /// ring it is, not the disc it would be without the bore.
    #[test]
    fn a_bored_mirror_is_meshed_around_its_bore() -> OpmResult<()> {
        let (radius, bore) = (millimeter!(12.5), millimeter!(4.0));
        let mesh = node_patches(&bored_mirror(0.0, 0.0)?, 64)?
            .into_iter()
            .find(|patch| patch.face == Some(Extruded::FRONT))
            .map(|patch| patch.mesh)
            .ok_or_else(|| OpossumError::Other("the mirrored face was not meshed".into()))?;
        let covered = mesh
            .triangles()
            .iter()
            .map(|triangle| {
                let [a, b, c] =
                    triangle.map(|index| mesh.points()[index as usize].map(|c| c.value));
                (b - a).cross(&(c - a)).norm() / 2.
            })
            .sum::<f64>();
        let ring_area = PI * (radius.value.powi(2) - bore.value.powi(2));
        assert_relative_eq!(covered, ring_area, max_relative = 1e-2);
        Ok(())
    }
    /// The mesh of an off-axis parabola lies on the mirror, every normal standing on it, although
    /// the optic shape is measured across the axis of the parent parabola rather than the mirror's
    /// own.
    #[test]
    fn the_mesh_of_an_off_axis_parabola_lies_on_the_mirror() -> OpmResult<()> {
        let mut parabola =
            ParabolicMirror::new_with_off_axis_x("oap", millimeter!(100.0), true, degree!(90.0))?;
        let optic_shape =
            OpticShape::Bounded(Outline::new(CircleShape::new(millimeter!(25.0))?, None)?);
        parabola.set_property(OPTIC_SHAPE, optic_shape.into())?;
        let mesh = surface_mesh(&parabola, 32)?;
        let geometry = parabola
            .create_geometry()?
            .ok_or_else(|| OpossumError::Other("a mirror has a geometry".into()))?;
        let face = geometry
            .face(SurfaceGeometry::FACE)
            .ok_or_else(|| OpossumError::Other("a mirror has a face".into()))?;
        for (point, normal) in mesh.points().iter().zip(mesh.normals()) {
            assert!(
                face.is_on_vertex_sheet(point),
                "{point:?} does not lie on the mirror"
            );
            let on_surface = face.frame().inverse_transform_point(point);
            let surface_normal = face
                .surface()
                .local_normal_at(&Point2::new(on_surface.x, on_surface.y))
                .expect("the mirror reaches a point it was meshed at");
            let surface_normal = face.frame().transform_vector_f64(&surface_normal);
            assert_abs_diff_eq!(normal.dot(&surface_normal).abs(), 1.0, epsilon = 1e-9);
        }
        Ok(())
    }
    /// A lens whose curvature radius exactly equals its clear aperture radius is a hemisphere - the
    /// tightest a spherical surface can be without folding back past its own equator. Geometrically
    /// this is exactly valid (the sag at the rim is the radius itself, the surface just reaches flat
    /// tangent to the aperture plane there), so this pins down whether it can actually be meshed, or
    /// whether the floating-point boundary in the sag of a sphere rejects it as "not reaching as
    /// far out as" the aperture.
    #[test]
    fn a_hemisphere_lens_can_be_meshed() -> OpmResult<()> {
        let hemisphere_radius = millimeter!(25.0);
        let aperture: Proptype = OpticShape::from(CircleShape::new(hemisphere_radius)?).into();

        // Plano-convex: one flat face, one exact hemisphere - the centre thickness has to reach
        // that one sag (25 mm) or the hemisphere's rim would poke out past the flat face.
        let mut plano_convex = Lens::new(
            "plano-convex hemisphere",
            millimeter!(f64::INFINITY),
            -hemisphere_radius,
            hemisphere_radius,
            RefrIndexConst::new(1.5)?,
        )?;
        plano_convex.set_property(OPTIC_SHAPE, aperture.clone())?;
        node_patches(&plano_convex, 64)?;

        // Bi-convex: both faces an exact hemisphere, so the centre thickness must reach both sags
        // (25 mm each) or the two surfaces would cross inside the lens.
        let mut bi_convex = Lens::new(
            "bi-convex hemisphere",
            hemisphere_radius,
            -hemisphere_radius,
            millimeter!(50.0),
            RefrIndexConst::new(1.5)?,
        )?;
        bi_convex.set_property(OPTIC_SHAPE, aperture)?;
        node_patches(&bi_convex, 64)?;
        Ok(())
    }
    /// The patches of a body lie on exactly its faces, the lateral face on none. A face added to
    /// the geometry, such as a wall that becomes a face of its own, has to be drawn on before this
    /// passes again.
    #[test]
    fn the_patches_of_a_body_lie_on_exactly_its_faces() -> OpmResult<()> {
        for (node_type, _) in node_types() {
            if Appearance::of_node_type(node_type)?.is_none() {
                continue;
            }
            let node = create_node_ref(node_type)?;
            let Some(geometry) = node.create_geometry()? else {
                continue;
            };
            if geometry.kind().as_body().is_none() {
                continue;
            }
            let faces: Vec<FaceIndex> = node_patches(&*node, 32)?
                .iter()
                .filter_map(|patch| patch.face)
                .collect();
            let all: Vec<FaceIndex> = (0..geometry.kind().face_count())
                .map(FaceIndex::new)
                .collect();
            assert_eq!(
                faces, all,
                "the patches of a {node_type} do not lie on exactly its faces"
            );
        }
        Ok(())
    }
}
