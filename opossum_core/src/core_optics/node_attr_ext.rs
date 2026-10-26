//! Extension trait for common optical node attributes.
//!
//! Provides non-overridable helper methods for querying and updating node attributes
//! on any type that implements [`HasNodeAttr`].

use crate::{
    core_optics::{NodeAttr, node_attr::HasNodeAttr, optic_surface::OpticSurface},
    error::OpmResult,
    geometry::FaceIndex,
    light::Rays,
    properties::{Properties, Proptype},
    utils::geom_transformation::Isometry,
};
use nalgebra::Point2;

/// Extension trait providing standardized, non-overridable methods
/// for checking and manipulating core node attributes.
pub trait NodeAttrExt {
    // --- Immutable Methods (Getters) ---

    /// Get the node type of this optical node.
    fn node_type(&self) -> &str;

    /// Get the name of this optical node.
    fn name(&self) -> &str;

    /// Get the 2D GUI position of this optical node on the frontend canvas.
    fn gui_position(&self) -> Option<Point2<f64>>;

    /// Return all custom properties of this optical node.
    fn properties(&self) -> &Properties;

    /// Returns `true` if the node should be analyzed in reverse direction.
    fn inverted(&self) -> bool;

    /// Get the local alignment (decenter, tilt) of an optical node.
    ///
    /// Returns `None` if no local alignment is defined for this node.
    fn alignment(&self) -> Option<Isometry>;

    /// Return a display string in the format `'<name>' (<node_type>)`.
    fn node_info(&self) -> String;

    /// Return the optic surfaces of this node that recorded hits, each with the name of its face
    /// (see [`GeometryKind::face_name`](crate::geometry::GeometryKind::face_name)), in the order
    /// the faces are numbered.
    fn surfaces_with_hits(&self) -> Vec<(String, &OpticSurface)>;

    // --- Mutable Methods (Setters / Actions) ---

    /// Set a custom property of this optical node.
    ///
    /// The property must already exist on the node.
    ///
    /// # Errors
    ///
    /// Returns an error if the property is undefined or if the value type does not match.
    fn set_property(&mut self, name: &str, proptype: Proptype) -> OpmResult<()>;

    /// Resets the ray caches and hit maps of all optical surfaces belonging to this node.
    fn reset_optic_surfaces(&mut self);

    /// Update node attributes of this node from the given [`NodeAttr`].
    ///
    /// Retains runtime surface instances while updating configuration and properties.
    ///
    /// # Errors
    ///
    /// Returns an error if attribute validation fails.
    fn set_node_attr(&mut self, node_attributes: NodeAttr) -> OpmResult<()>;
}

/// Blanket implementation for any type that provides access to [`NodeAttr`].
impl<T: ?Sized + HasNodeAttr> NodeAttrExt for T {
    fn node_type(&self) -> &str {
        self.node_attr().node_type()
    }

    fn name(&self) -> &str {
        self.node_attr().name()
    }

    fn gui_position(&self) -> Option<Point2<f64>> {
        self.node_attr().gui_position()
    }

    fn properties(&self) -> &Properties {
        self.node_attr().properties()
    }

    fn inverted(&self) -> bool {
        self.node_attr().inverted()
    }

    fn alignment(&self) -> Option<Isometry> {
        *self.node_attr().alignment()
    }

    fn node_info(&self) -> String {
        format!("'{}' ({})", self.name(), self.node_type())
    }

    fn surfaces_with_hits(&self) -> Vec<(String, &OpticSurface)> {
        let Some(geometry) = self.node_attr().runtime_geometry() else {
            return Vec::new();
        };
        geometry
            .surfaces()
            .filter(|(_, surface)| !surface.hit_map().is_empty())
            .filter_map(|(face, surface)| Some((geometry.kind().face_name(face)?, surface)))
            .collect()
    }

    fn set_property(&mut self, name: &str, proptype: Proptype) -> OpmResult<()> {
        self.node_attr_mut().set_property(name, proptype)
    }

    fn reset_optic_surfaces(&mut self) {
        let Some(geometry) = self.node_attr_mut().runtime_geometry_mut() else {
            return;
        };
        let kind = geometry.kind_mut();
        for face in (0..kind.face_count()).map(FaceIndex::new) {
            if let Some(optic_surf) = kind.surface_mut(face) {
                optic_surf.set_backwards_rays_cache(Vec::<Rays>::new());
                optic_surf.set_forward_rays_cache(Vec::<Rays>::new());
                optic_surf.reset_hit_map();
            }
        }
    }

    fn set_node_attr(&mut self, node_attributes: NodeAttr) -> OpmResult<()> {
        let node_attr_mut = self.node_attr_mut();

        // Set or clear alignment explicitly to prevent retaining stale values
        node_attr_mut.set_alignment_option(*node_attributes.alignment());
        node_attr_mut.set_positioning(*node_attributes.positioning());
        node_attr_mut.set_name(node_attributes.name());
        node_attr_mut.set_inverted(node_attributes.inverted());

        if let Some((node_idx, distance)) = node_attributes.get_align_like_node_at_distance() {
            node_attr_mut.set_align_like_node_at_distance(*node_idx, *distance);
        }

        node_attr_mut.update_properties(node_attributes.properties());
        node_attr_mut.set_ports(node_attributes.raw_ports().clone());
        node_attr_mut.set_uuid(node_attributes.uuid());
        node_attr_mut.set_gui_position(node_attributes.gui_position());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{core_optics::node_attr::NodePositioning, millimeter, nodes::Dummy};

    #[test]
    fn node_info_formatting() {
        let node = Dummy::default();
        assert_eq!(node.node_info(), "'dummy' (dummy)");
    }

    #[test]
    fn set_node_attr_clears_alignment_when_none() -> OpmResult<()> {
        let mut node = Dummy::default();
        let align_iso = Isometry::new_along_z(millimeter!(10.0))?;
        node.node_attr_mut().set_alignment(align_iso);
        assert!(node.alignment().is_some());

        // New attributes with alignment: None
        let mut new_attrs = NodeAttr::new("dummy");
        new_attrs.set_positioning(NodePositioning::Absolute(align_iso));
        assert!(new_attrs.alignment().is_none());

        node.set_node_attr(new_attrs)?;
        assert_eq!(node.alignment(), None, "Alignment must be cleared");
        assert_eq!(
            node.node_attr().positioning(),
            &NodePositioning::Absolute(align_iso)
        );
        Ok(())
    }
}
