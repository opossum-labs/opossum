#![warn(missing_docs)]
//! The capability of an optical node to be one surface rather than a body of material.
//!
//! A [`GeoSurface`](crate::geometry::geo_surface::GeoSurface) is where light refracts or reflects.
//! Some nodes — a mirror, a grating, a filter, a detector — are nothing but one such surface: there
//! is no second surface and no medium in between, unlike a lens or a wedge (see
//! [`Volumetric`](crate::core_optics::Volumetric)).
//!
//! [`Planar`] is what states that. It is deliberately a capability of the node rather than a list of
//! node type names kept somewhere else, for the same reason [`Volumetric`] is: nodes are handled as
//! `dyn Analyzable` trait objects throughout the graph, so a caller that only holds such an object
//! cannot recover the concrete type it once was. [`OpticNode::as_surface`] is the one question that
//! survives that erasure — every node answers it, and only those that are one surface answer it with
//! `Some`.

use crate::{
    core_optics::{Appearance, NodeAttrExt, OpticNode, OpticNodeExt, SurfaceFinish},
    error::{OpmResult, OpossumError},
    geometry::{Rim, RimRole, SurfaceMesh, body::default_optic_shape},
    utils::geom_transformation::Isometry,
};

/// An [`OpticNode`] that is one optical surface rather than a body of material.
///
/// Implementing this trait is what makes a node a surface node. Everything the drawing machinery
/// needs is derived from what such a node already has — the single
/// [`GeoSurface`](crate::geometry::geo_surface::GeoSurface) every one of its ports shares (see e.g.
/// `ThinMirror::update_surfaces`) and the [`Rim`] that surface ends at, its clear aperture placed
/// in space — so a node type declares the capability with an `impl Planar for ... {}` that answers
/// only [`Planar::surface_kind`], plus the [`OpticNode::as_surface`] override that makes it visible
/// through a trait object.
pub trait Planar: OpticNode {
    /// How this node's surface should be drawn.
    ///
    /// A node type answers this about itself rather than a shared file guessing from its name: a
    /// mirror, a grating and a detector all implement [`Planar`] alike, but they are not the same
    /// kind of surface to look at, and the compiler makes sure a new [`Planar`] node type has to
    /// decide which it is instead of silently falling back to some default.
    fn surface_kind(&self) -> SurfaceKind;

    /// The cosmetic appearance of this surface for 3-D rendering.
    ///
    /// This method has a default implementation derived from [`Planar::surface_kind`], whereas
    /// `surface_kind` itself has no default.  The asymmetry is deliberate:
    ///
    /// * `surface_kind` governs analysers and must be answered explicitly by every new
    ///   implementor so the compiler forces the decision.
    /// * `appearance` is purely cosmetic — a wrong value only looks wrong in the rendered scene
    ///   and never miscomputes anything — so a sensible default based on `surface_kind` is safe.
    ///
    /// Override this method only when a node needs a non-standard look.  For example, a
    /// [`ReflectiveGrating`](crate::nodes::ReflectiveGrating) is physically
    /// [`SurfaceKind::Reflective`] but looks
    /// [`SurfaceFinish::Iridescent`](crate::core_optics::SurfaceFinish::Iridescent).
    ///
    /// # Returns
    ///
    /// An [`Appearance`] derived from the node's [`SurfaceKind`], or a node-specific override.
    fn appearance(&self) -> Appearance {
        match self.surface_kind() {
            SurfaceKind::Reflective => Appearance {
                color: [0.9, 0.9, 0.92, 1.0],
                roughness: 0.05,
                finish: SurfaceFinish::Reflective,
            },
            SurfaceKind::Transmissive => Appearance {
                color: [0.85, 0.9, 0.85, 0.4],
                roughness: 0.5,
                finish: SurfaceFinish::Transmissive,
            },
            SurfaceKind::Detector => Appearance {
                color: [0.25, 0.25, 0.28, 1.0],
                roughness: 0.7,
                finish: SurfaceFinish::Opaque,
            },
        }
    }

    /// Mesh this node's one surface within its [`Rim`], so that what is drawn is what light hits.
    ///
    /// An unbounded surface, one whose clear aperture is open, has no rim; it is meshed over the
    /// default clear aperture, as a nominal size to draw it at.
    ///
    /// The counterpart of [`Volumetric::volume_body`](crate::core_optics::Volumetric::volume_body)
    /// for a node with one surface instead of two: the surface is built from the node's
    /// [`Geometry`](crate::geometry::Geometry) at its current placement, so the mesh follows the
    /// properties as they are now even if the node's surfaces have not been rebuilt since.
    ///
    /// # Arguments
    ///
    /// * `segments` - how finely the rim is sampled; see e.g. `RIM_SEGMENTS` in the backend's scene
    ///   export, which is what this exists for.
    ///
    /// # Returns
    ///
    /// The meshed surface, in the node's own frame (see [`SurfaceMesh`]).
    ///
    /// # Errors
    ///
    /// This function returns an error if the node has no geometry, if its geometry cannot be
    /// derived or built, or if the surface cannot be meshed within its rim (too few segments, a rim
    /// without a closed edge such as a ring, or a surface that cannot reach as far out as the rim).
    fn surface_mesh(&self, segments: usize) -> OpmResult<SurfaceMesh> {
        let Some(geometry) = self.geometry()? else {
            return Err(OpossumError::Other(format!(
                "node '{}' has no geometry to draw",
                self.name()
            )));
        };
        let node_frame = self.effective_node_iso().unwrap_or_else(Isometry::identity);
        let placed = geometry.place(&node_frame)?;
        let rim = match placed.rim() {
            Some(rim) => rim.clone(),
            None => Rim::new(
                default_optic_shape().cross_section()?.ok_or_else(|| {
                    OpossumError::Other("the default optic shape bounds no region".into())
                })?,
                Isometry::identity(),
                RimRole::Edge,
            ),
        };
        // The cross section lies across the rim's axis, which need not be the node's: the surface
        // is meshed along that axis and the result is then stated in the node's frame.
        let axis = rim.frame(&Isometry::identity());
        let aperture_mesh = rim.cross_section().get().triangulate(segments)?;
        Ok(placed
            .entrance()
            .0
            .mesh_over(&aperture_mesh, &node_frame.append(&axis))?
            .transformed(&axis))
    }
}

/// How a [`Planar`] node's surface should be drawn.
///
/// A plain fact about the node type, not a rendering decision made elsewhere: see
/// [`Planar::surface_kind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
    /// A surface light reflects from: a mirror, a grating, or a beam splitter's plate.
    Reflective,
    /// A surface light passes through without entering a body of material: an idealised filter or a
    /// paraxial element.
    Transmissive,
    /// A surface that measures or otherwise terminates light rather than redirecting it: a detector.
    Detector,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Bore, CircleShape, OpticShape, Outline},
        core_optics::node_attr::NodePositioning,
        degree,
        geometry::body::OPTIC_SHAPE,
        millimeter,
        nodes::{EnergyMeter, IdealFilter, ParabolicMirror, ThinMirror},
        properties::Proptype,
        utils::geom_transformation::Isometry,
    };
    use approx::{assert_abs_diff_eq, assert_relative_eq};
    use nalgebra::Point2;
    use std::f64::consts::PI;

    /// A node in its default state — a flat mirror over the default clear aperture — can be meshed
    /// at all, and the mesh actually covers ground rather than coming back empty.
    #[test]
    fn a_planar_node_can_be_meshed_in_its_default_state() -> OpmResult<()> {
        let mut mirror = ThinMirror::default();
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let mesh = mirror.surface_mesh(32)?;
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
        let mesh = mirror.surface_mesh(32)?;
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
            mirror.surface_mesh(16)?
        };
        let far_away = {
            let mut mirror = ThinMirror::default();
            mirror.set_positioning(NodePositioning::Absolute(Isometry::new(
                crate::millimeter!(0.0, 0.0, 500.0),
                crate::degree!(0.0, 0.0, 0.0),
            )?))?;
            mirror.surface_mesh(16)?
        };
        assert_eq!(
            at_origin.points(),
            far_away.points(),
            "the surface mesh must be independent of the node's placement"
        );
        Ok(())
    }

    /// A mirror with a bore through it is meshed around the bore: what is drawn covers the ring it
    /// is, not the disc it would be without the bore.
    #[test]
    fn a_surface_with_a_bore_is_meshed_around_it() -> OpmResult<()> {
        let (radius, bore) = (millimeter!(12.5), millimeter!(4.0));
        let ring = OpticShape::Bounded(Outline::new(
            CircleShape::new(radius)?,
            vec![Bore::new(
                CircleShape::new(bore)?,
                millimeter!(0., 0.),
                degree!(0.),
            )],
        )?);
        let mut mirror = ThinMirror::default();
        mirror.set_property(OPTIC_SHAPE, ring.into())?;
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let mesh = mirror.surface_mesh(64)?;
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

    /// What is drawn is what is hit. The clear aperture of an off-axis parabola is measured along
    /// the parent axis, not along the node's, so its mesh has to lie on the mirror all over and
    /// reach exactly as far as its rim does.
    #[test]
    fn the_mesh_of_an_off_axis_parabola_fills_its_rim() -> OpmResult<()> {
        let mut parabola =
            ParabolicMirror::new_with_off_axis_x("oap", millimeter!(100.0), true, degree!(90.0))?;
        let optic_shape =
            OpticShape::Bounded(Outline::new(CircleShape::new(millimeter!(25.0))?, vec![])?);

        parabola.set_property(OPTIC_SHAPE, optic_shape.into())?;
        // At the origin the node's own frame, which the mesh is stated in, is the global one.
        parabola.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let mesh = parabola.surface_mesh(32)?;

        let surface = parabola
            .get_optic_surface("input_1")
            .expect("a mirror has a surface at its input");
        let rim_frame = surface
            .rim()
            .expect("a mirror is bounded")
            .frame(&Isometry::identity());
        let geo_surface = surface.geo_surface();
        let geo_surface = &*geo_surface.0;
        let mut reach = 0.0_f64;
        for point in mesh.points() {
            assert!(
                geo_surface.is_on_vertex_sheet(point),
                "{point:?} does not lie on the mirror"
            );
            let across = rim_frame.inverse_transform_point(point);
            reach = reach.max(across.x.value.hypot(across.y.value));
        }
        assert_abs_diff_eq!(reach, millimeter!(25.0).value, epsilon = 1e-9);
        // Each normal stands on the mirror where its point lies, whichever side it faces.
        for (point, normal) in mesh.points().iter().zip(mesh.normals()) {
            let on_surface = geo_surface.isometry().inverse_transform_point(point);
            let surface_normal = geo_surface
                .local_normal_at(&Point2::new(on_surface.x, on_surface.y))
                .expect("the mirror reaches a point it was meshed at");
            let surface_normal = geo_surface.isometry().transform_vector_f64(&surface_normal);
            assert_abs_diff_eq!(normal.dot(&surface_normal).abs(), 1.0, epsilon = 1e-9);
        }
        Ok(())
    }

    /// A `ThinMirror` (physically Reflective) gets a Reflective finish by default.
    #[test]
    fn reflective_surface_kind_gives_reflective_finish() {
        let mirror = ThinMirror::default();
        let app = mirror.appearance();
        assert_eq!(
            app.finish,
            SurfaceFinish::Reflective,
            "a ThinMirror must default to SurfaceFinish::Reflective"
        );
        assert_eq!(
            app.color,
            [0.9, 0.9, 0.92, 1.0_f32],
            "reflective surface colour mismatch"
        );
    }

    /// An `IdealFilter` (physically Transmissive) gets a Transmissive finish by default.
    #[test]
    fn transmissive_surface_kind_gives_transmissive_finish() {
        let filter = IdealFilter::default();
        let app = filter.appearance();
        assert_eq!(
            app.finish,
            SurfaceFinish::Transmissive,
            "an IdealFilter must default to SurfaceFinish::Transmissive"
        );
        assert_eq!(
            app.color,
            [0.85, 0.9, 0.85, 0.4_f32],
            "transmissive surface colour mismatch"
        );
    }

    /// An `EnergyMeter` (physically Detector) gets an Opaque finish by default.
    #[test]
    fn detector_surface_kind_gives_opaque_finish() {
        let meter = EnergyMeter::default();
        let app = meter.appearance();
        assert_eq!(
            app.finish,
            SurfaceFinish::Opaque,
            "an EnergyMeter must default to SurfaceFinish::Opaque"
        );
        assert_eq!(
            app.color,
            [0.25, 0.25, 0.28, 1.0_f32],
            "detector surface colour mismatch"
        );
    }
}
