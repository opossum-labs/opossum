#![warn(missing_docs)]
//! Contains the basic trait representing an optical element.

use std::any::Any;

use log::warn;
use nalgebra::Point3;
use uom::si::f64::{Angle, Length};
use uuid::Uuid;

use crate::{
    analyzers::{Analyzable, AnalyzerKind, propagation_strategy::PropagationStrategy},
    core_optics::{
        NodeAttrExt, OpticPorts, PortType,
        node_attr::{HasNodeAttr, NodePositioning},
        planar::Planar,
        volumetric::Volumetric,
    },
    error::OpmResult,
    light::LightData,
    nodes::fluence_detector::Fluence,
    reporting::{Dottable, node_report::NodeReportResult},
    utils::geom_transformation::Isometry,
};

/// Helper trait for dynamic downcasting of optical nodes.
///
/// This trait is automatically implemented by the `#[derive(OpmNode)]` macro.
pub trait OpticNodeAny {
    /// Returns an immutable reference to `Any` for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Returns a mutable reference to `Any` for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// This is the basic trait that must be implemented by all concrete optical components.
pub trait OpticNode: Dottable + HasNodeAttr + OpticNodeAny {
    /// Sets the apodization warning on nodes that have that attribute.
    fn set_apodization_warning(&mut self, _apodized: bool) {
        warn!(
            "\"set_apodization_warning\" is not implemented for '{}' ({})",
            self.name(),
            self.node_type()
        );
    }

    /// Hook to store light data during analysis.
    ///
    /// Overridden by detector nodes to capture passing light data for reports.
    fn set_light_data(&mut self, _ld: Option<LightData>) {}

    /// Sets the 3D positioning strategy of this optical node and updates optical surfaces.
    ///
    /// # Errors
    ///
    /// This function returns an error if the [`update_surfaces`](OpticNode::update_surfaces) function fails.
    fn set_positioning(&mut self, positioning: NodePositioning) -> OpmResult<()> {
        self.node_attr_mut().set_positioning(positioning);
        self.update_surfaces()
    }

    /// Caches a calculated runtime isometry if this node is in automatic positioning mode,
    /// and updates optical surfaces accordingly.
    ///
    /// Does nothing if the node is configured with an absolute position.
    ///
    /// # Errors
    ///
    /// This function returns an error if the [`update_surfaces`](OpticNode::update_surfaces) function fails.
    fn set_cached_isometry(&mut self, iso: Isometry) -> OpmResult<()> {
        self.node_attr_mut().set_cached_isometry(iso);
        self.update_surfaces()
    }

    /// Resets internal data (e.g., captured light data in detectors and runtime medium states).
    fn reset_data(&mut self) {
        self.set_light_data(None);
        self.reset_optic_surfaces();
        self.node_attr_mut().clear_runtime_inversion();
    }

    /// Prepare this node's medium before the first ray is traced (Phase A).
    ///
    /// Derives the node's [`SurfaceBoundedBody`](crate::geometry::body::SurfaceBoundedBody) from the
    /// **current** node isometry and stores it in [`NodeAttr`](crate::core_optics::node_attr::NodeAttr)'s
    /// `runtime_medium` slot so that [`Volumetric::propagate_inside_medium`] (Phase B) can read it without
    /// rebuilding per ray pass. If the operating point provides a gain model that reads the inversion, the
    /// [`InversionField`](crate::gain::InversionField) is built from the pump configuration and stored
    /// alongside the body; otherwise the inversion slot is `None`.
    ///
    /// The body is always re-derived on every call, so geometry edits (e.g. changed centre thickness)
    /// and repositioning by the analyzer's `calc_node_positions` are picked up correctly. Non-volume nodes
    /// return immediately without touching the medium slot.
    ///
    /// [`NodeGroup`](crate::nodes::NodeGroup) overrides this to recurse into every child node.
    ///
    /// # Errors
    ///
    /// Returns an error if the body cannot be derived from the node's surfaces or if building the
    /// inversion field fails.
    fn prepare_volume(&mut self, strategy: &dyn PropagationStrategy) -> OpmResult<()> {
        let Some(volumetric) = self.as_volume() else {
            return Ok(());
        };
        // The body is pure geometry — build it for every volume node regardless of the operating
        // point, so it is available whenever anything needs to query the medium.
        let body = volumetric.volume_body()?;
        let config = strategy.pump_config(self.node_attr().uuid());
        // The inversion is gain-specific: only build it when the gain model reads one.
        let inversion = match config.gain_model().as_extraction() {
            Some(model) => model.build_inversion(&body, &config)?,
            None => None,
        };
        self.node_attr_mut().set_runtime_medium(body, inversion);
        Ok(())
    }

    /// Hook invoked immediately after deserialization (e.g. when loaded from an `.opm` file).
    ///
    /// By default, this function triggers [`update_surfaces`](OpticNode::update_surfaces) to reconstruct
    /// the runtime surface geometry.
    ///
    /// # Errors
    ///
    /// Returns an error if reconstructing the optical surfaces fails.
    fn after_deserialization_hook(&mut self) -> OpmResult<()> {
        self.update_surfaces()?;
        Ok(())
    }

    /// Updates the optical surfaces of this node according to its current geometry and positioning.
    ///
    /// # Errors
    ///
    /// Returns an error if geometry evaluation or surface construction fails.
    fn update_surfaces(&mut self) -> OpmResult<()>;

    /// Return the effective (input & output) ports of this [`OpticNode`].
    ///
    /// Accounts for node inversion if the node is configured in reverse orientation.
    fn ports(&self) -> OpticPorts {
        let mut ports = self.node_attr().raw_ports().clone();
        if self.node_attr().inverted() {
            ports.set_inverted(true);
        }
        ports
    }

    /// Return the 3D positioning strategy of this optical node.
    fn positioning(&self) -> &NodePositioning {
        self.node_attr().positioning()
    }

    /// Convenience getter for the effective isometry (absolute or cached automatic).
    fn effective_position(&self) -> Option<&Isometry> {
        self.node_attr().effective_position()
    }

    /// Sets whether this [`OpticNode`] is inverted in the optical path.
    ///
    /// Inversion signifies that light propagates through this element in reverse order.
    ///
    /// # Errors
    ///
    /// Returns an error if the node cannot be inverted (e.g., source nodes or groups containing sources).
    fn set_inverted(&mut self, inverted: bool) -> OpmResult<()> {
        self.node_attr_mut().set_inverted(inverted);
        Ok(())
    }

    /// Return this node as a [`Volumetric`] element, if it encloses a volume of material.
    ///
    /// # Returns
    ///
    /// `Some(&dyn Volumetric)` if the element represents a medium, or `None` otherwise.
    fn as_volume(&self) -> Option<&dyn Volumetric> {
        None
    }

    /// Return this node as a [`Planar`] element, if it is one optical surface rather than a body of
    /// material.
    ///
    /// # Returns
    ///
    /// `Some(&dyn Planar)` if the element is a single surface, or `None` otherwise.
    fn as_surface(&self) -> Option<&dyn Planar> {
        None
    }

    /// Return [`NodeReportResult`] of the current state of this [`OpticNode`].
    ///
    /// Override this function in detector nodes to provide analysis output.
    /// By default, [`NodeReportResult::None`] is returned.
    ///
    /// # Errors
    ///
    /// Returns an error if the concrete report calculation fails.
    fn node_report(&self, _uuid: &str, _analyzer: AnalyzerKind) -> OpmResult<NodeReportResult> {
        Ok(NodeReportResult::None)
    }
}

/// Helper trait for optical elements that can be locally aligned.
pub trait Alignable: OpticNode + Sized {
    /// Locally decenter an optical element.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given `decenter` coordinates contain non-finite values.
    fn with_decenter(mut self, decenter: Point3<Length>) -> OpmResult<Self> {
        let old_rotation = self
            .positioning()
            .effective_position()
            .map_or_else(Point3::origin, Isometry::rotation);
        let translation_iso = Isometry::new(decenter, old_rotation)?;
        self.node_attr_mut().set_alignment(translation_iso);
        Ok(self)
    }

    /// Locally tilt an optical element.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given `tilt` angles contain non-finite values.
    fn with_tilt(mut self, tilt: Point3<Angle>) -> OpmResult<Self> {
        let old_translation = self
            .positioning()
            .effective_position()
            .map_or_else(Point3::origin, Isometry::translation);
        let rotation_iso = Isometry::new(old_translation, tilt)?;
        self.node_attr_mut().set_alignment(rotation_iso);
        Ok(self)
    }

    /// Aligns this optical element with respect to another optical element.
    ///
    /// Specifically, the center (optical) axes of these two nodes are aligned and their anchor points
    /// are separated by a given distance.
    ///
    /// # Returns
    ///
    /// This function returns the original node with updated alignment settings.
    #[must_use]
    fn align_like_node_at_distance(mut self, node_id: Uuid, distance: Length) -> Self {
        self.node_attr_mut()
            .set_align_like_node_at_distance(node_id, distance);
        self
    }
}

/// Trait to define a Laser-Induced Damage Threshold (LIDT) for a node.
pub trait LIDT: OpticNode + Analyzable + Sized {
    /// Sets an LIDT value for all ports of this node.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given LIDT is negative or NaN,
    /// or if updating runtime surfaces fails.
    fn with_lidt(mut self, lidt: Fluence) -> OpmResult<Self> {
        let mut ports = self.ports();
        let in_ports = ports.names(&PortType::Input);
        let out_ports = ports.names(&PortType::Output);

        for port_name in &in_ports {
            ports.set_lidt(&PortType::Input, port_name, lidt)?;
        }
        for port_name in &out_ports {
            ports.set_lidt(&PortType::Output, port_name, lidt)?;
        }

        self.node_attr_mut().set_ports(ports);
        self.update_surfaces()?; // Required to propagate the port update to runtime surfaces
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;

    use super::*;
    use crate::{
        core_optics::OpticNodeExt,
        degree,
        error::{OpossumError, assert_err},
        millimeter,
        nodes::Dummy,
    };

    #[test]
    fn set_alignment() -> OpmResult<()> {
        let mut node = Dummy::default();
        let decenter = millimeter!(1.0, 2.0, 3.0);
        let tilt = degree!(0.1, 0.2, 0.3);
        assert!(node.set_alignment(decenter, tilt).is_ok());
        let alignment = node
            .node_attr()
            .alignment()
            .clone()
            .ok_or_else(|| OpossumError::Other("Error getting alignment".to_string()))?;
        assert_abs_diff_eq!(alignment.translation().x.value, decenter.x.value);
        assert_abs_diff_eq!(alignment.translation().y.value, decenter.y.value);
        assert_abs_diff_eq!(alignment.translation().z.value, decenter.z.value);
        assert_abs_diff_eq!(alignment.rotation().x.value, tilt.x.value);
        assert_abs_diff_eq!(alignment.rotation().y.value, tilt.y.value);
        assert_abs_diff_eq!(alignment.rotation().z.value, tilt.z.value);
        Ok(())
    }

    #[test]
    fn effective_node_iso() -> OpmResult<()> {
        let mut node = Dummy::default();
        let decenter = millimeter!(1.0, 2.0, 3.0);
        let tilt = degree!(0.0, 0.0, 0.0);
        let iso = Isometry::new(decenter, tilt)?;
        node.set_positioning(NodePositioning::Absolute(iso))?;
        let local_trans = millimeter!(4.0, 5.0, 6.0);
        node.set_alignment(local_trans, degree!(0.0, 0.0, 0.0))?;
        let iso = node
            .effective_node_iso()
            .ok_or_else(|| OpossumError::OpmDocument("Error getting effective iso".to_string()))?;
        assert_abs_diff_eq!(
            iso.translation().x.value,
            decenter.x.value + local_trans.x.value
        );
        assert_abs_diff_eq!(
            iso.translation().y.value,
            decenter.y.value + local_trans.y.value
        );
        assert_abs_diff_eq!(
            iso.translation().z.value,
            decenter.z.value + local_trans.z.value
        );
        Ok(())
    }

    #[test]
    fn effective_surface_iso() -> OpmResult<()> {
        let mut node = Dummy::default();
        let decenter = millimeter!(1.0, 2.0, 3.0);
        let tilt = degree!(0.1, 0.2, 0.3);
        node.set_alignment(decenter, tilt)?;
        assert_err(
            node.effective_surface_iso("input_1"),
            OpossumError::Other("no effective node iso defined".to_string()),
        );

        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        assert_err(
            node.effective_surface_iso("wrong"),
            OpossumError::Other("no surface with name wrong defined".to_string()),
        );
        let iso = node.effective_surface_iso("input_1")?;
        assert_abs_diff_eq!(iso.translation().x.value, decenter.x.value);
        assert_abs_diff_eq!(iso.translation().y.value, decenter.y.value);
        assert_abs_diff_eq!(iso.translation().z.value, decenter.z.value);
        Ok(())
    }
}
