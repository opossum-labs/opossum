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
    apertures::{Aperture, ApertureType},
    core_optics::{NodeAttrExt, OpticNode, OpticNodeExt, PortType},
    error::{OpmResult, OpossumError},
    geometry::{SurfaceMesh, body::CLEAR_APERTURE},
    properties::Proptype,
    utils::geom_transformation::Isometry,
};

/// An [`OpticNode`] that is one optical surface rather than a body of material.
///
/// Implementing this trait is what makes a node a surface node. Everything the drawing machinery
/// needs is derived from what such a node already has — the single
/// [`GeoSurface`](crate::geometry::geo_surface::GeoSurface) every one of its ports shares (see e.g.
/// `ThinMirror::update_surfaces`) and its [`CLEAR_APERTURE`] property for that surface's transversal
/// extent — so a node type declares the capability with an `impl Planar for ... {}` that answers
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

    /// Mesh this node's one surface over its [`CLEAR_APERTURE`].
    ///
    /// The counterpart of [`Volumetric::volume_body`](crate::core_optics::Volumetric::volume_body)
    /// for a node with one surface instead of two: everything needed is already on the node, so this
    /// has a default implementation and no node type has to provide its own.
    ///
    /// # Arguments
    ///
    /// * `segments` - how finely the rim of the aperture is sampled; see e.g. `RIM_SEGMENTS` in the
    ///   backend's scene export, which is what this exists for.
    ///
    /// # Returns
    ///
    /// The meshed surface, in the node's own frame (see [`SurfaceMesh`]).
    ///
    /// # Errors
    ///
    /// This function returns an error if the node has no input port to read a surface from, if that
    /// surface is not registered, if the node declares no [`CLEAR_APERTURE`], or if the surface
    /// cannot be meshed over it (too few segments, or a surface that cannot reach as far out as the
    /// aperture).
    fn surface_mesh(&self, segments: usize) -> OpmResult<SurfaceMesh> {
        // Every `Planar` node registers its one surface under all of its ports alike (a mirror under
        // `input_1` and `output_1`, a beam splitter under all four) - see e.g.
        // `ThinMirror::update_surfaces`. The first input port therefore always names it, regardless
        // of how many ports the node actually has.
        let surf_name = self
            .ports()
            .names(&PortType::Input)
            .first()
            .cloned()
            .ok_or_else(|| {
                OpossumError::Analysis(format!(
                    "node '{}' has no input port to read its surface from",
                    self.name()
                ))
            })?;
        let surface = self.get_optic_surface(&surf_name).ok_or_else(|| {
            OpossumError::Other(format!(
                "no surface with name {surf_name} defined for node '{}'",
                self.name()
            ))
        })?;
        let Ok(Proptype::Aperture(clear_aperture)) = self.node_attr().get_property(CLEAR_APERTURE)
        else {
            return Err(OpossumError::Other(format!(
                "node '{}' has no '{CLEAR_APERTURE}' property, so the extent of its surface is \
                 unknown",
                self.name()
            )));
        };
        let aperture_mesh = Aperture::new(clear_aperture.clone(), ApertureType::Hole, None, None)?
            .triangulate(segments)?;
        let frame = self.effective_node_iso().unwrap_or_else(Isometry::identity);
        surface.geo_surface().mesh_over(&aperture_mesh, &frame)
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
        core_optics::node_attr::NodePositioning, nodes::ThinMirror,
        utils::geom_transformation::Isometry,
    };

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
}
