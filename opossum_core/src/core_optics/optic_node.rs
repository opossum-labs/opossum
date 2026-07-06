#![warn(missing_docs)]
//! Contains the basic trait representing an optical element
use log::warn;
use nalgebra::Point3;
use uom::si::f64::{Angle, Length};
use uuid::Uuid;

use crate::core_optics::{NodeAttrExt, OpticPorts};
use crate::{
    analyzers::Analyzable,
    core_optics::{PortType, SceneryResources, node_attr::HasNodeAttr},
    error::OpmResult,
    light::LightData,
    nodes::fluence_detector::Fluence,
    reporting::{Dottable, node_report::NodeReport},
    utils::geom_transformation::Isometry,
};
use std::{
    any::Any,
    sync::{Arc, Mutex},
};

/// Helper trait for dynamic downcasting of optical nodes.
/// This trait is automatically implemented by the `#[derive(OpmNode)]` macro.
pub trait OpticNodeAny {
    /// Returns an immutable reference to `Any` for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Returns a mutable reference to `Any` for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// This is the basic trait that must be implemented by all concrete optical components.
pub trait OpticNode: Dottable + HasNodeAttr + OpticNodeAny {
    /// Sets the apodization warning on nodes that have that attribute
    fn set_apodization_warning(&mut self, _apodized: bool) {
        warn!(
            "\"set_apodization_warning\" is not implemented for '{}' ({})",
            self.name(),
            self.node_type()
        );
    }
    /// Hook to store light data during analysis.
    /// Overridden by detector nodes to capture passing data for reports.
    fn set_light_data(&mut self, _ld: Option<LightData>) {}
    /// Set the (base) [`Isometry`] (position and angle) of this optical node.
    ///
    /// # Errors
    /// This function errors if the `update_surfaces` function fails
    fn set_isometry(&mut self, isometry: Isometry) -> OpmResult<()> {
        self.node_attr_mut().set_isometry(isometry);
        self.update_surfaces()
    }
    /// Reset internal data (e.g. internal state of detector nodes)
    fn reset_data(&mut self) {
        self.set_light_data(None);
        self.reset_optic_surfaces();
    }
    /// This function is called right after a node has been deserialized (e.g. read from a file). By default, this
    /// function does nothing and returns no error.
    ///
    /// Currently this function is needed for group nodes whose internal graph structure must be synchronized with the
    /// graph stored in their properties.
    ///
    /// # Errors
    /// This function will return an error if the overwritten function generates an error.
    fn after_deserialization_hook(&mut self) -> OpmResult<()> {
        self.update_surfaces()?;
        Ok(())
    }
    /// Updates the surfaces of this node after deserialization
    ///
    /// # Errors
    ///
    /// This function might return an error in a non-default implementation
    fn update_surfaces(&mut self) -> OpmResult<()>;
    /// Return the available (input & output) ports of this [`OpticNode`].
    fn ports(&self) -> OpticPorts {
        let mut ports = self.node_attr().raw_ports().clone();
        if self.node_attr().inverted() {
            ports.set_inverted(true);
        }
        ports
    }
    /// Return the (base) [`Isometry`] of this optical node.
    fn isometry(&self) -> Option<Isometry> {
        self.node_attr().isometry()
    }
    /// Set the global configuration for this [`OpticNode`].
    /// **Note**: This function should normally only be used internally by `OpticRef`.
    fn set_global_conf(&mut self, global_conf: Option<Arc<Mutex<SceneryResources>>>) {
        self.node_attr_mut().set_global_conf(global_conf);
    }
    /// Set this [`OpticNode`] as inverted.
    ///
    /// This flag signifies that the [`OpticNode`] should be propagated in reverse order. This function normally simply sets the
    /// `inverted` property. For [`NodeGroup`](crate::nodes::NodeGroup) it also sets the `inverted` flag of the underlying `OpticGraph`.
    ///
    /// # Errors
    /// This function returns an error, if the node cannot be inverted. This is the case, if
    ///   - it is a source node
    ///   - it is a group node containing a non-invertable node (e.g. a source)
    fn set_inverted(&mut self, inverted: bool) -> OpmResult<()> {
        self.node_attr_mut().set_inverted(inverted);
        Ok(())
    }
    /// Return [`NodeReport`] of the current state of this [`OpticNode`].
    ///
    /// This function must be overridden for generating output in the analysis report. Mainly
    /// detector nodes use this feature. By default `None` is returned, signalling that a node does not
    /// provide a report at all.
    ///
    /// # Errors
    ///
    /// This function might return an error if the concrete implementations fail.
    fn node_report(&self, _uuid: &str) -> OpmResult<Option<NodeReport>> {
        Ok(None)
    }
}
/// Helper trait for optical elements that can be locally aligned
pub trait Alignable: OpticNode + Sized {
    /// Locally decenter an optical element.
    ///
    /// # Errors
    /// This function will return an error if the given `decenter` values are not finite.
    fn with_decenter(mut self, decenter: Point3<Length>) -> OpmResult<Self> {
        let old_rotation = self
            .isometry()
            .as_ref()
            .map_or_else(Point3::origin, Isometry::rotation);
        let translation_iso = Isometry::new(decenter, old_rotation)?;
        self.node_attr_mut().set_alignment(translation_iso);
        Ok(self)
    }
    /// Locally tilt an optical element.
    ///
    /// # Errors
    /// This function will return an error if the given `decenter` values are not finite.
    fn with_tilt(mut self, tilt: Point3<Angle>) -> OpmResult<Self> {
        let old_translation = self
            .isometry()
            .as_ref()
            .map_or_else(Point3::origin, Isometry::translation);
        let rotation_iso = Isometry::new(old_translation, tilt)?;
        self.node_attr_mut().set_alignment(rotation_iso);
        Ok(self)
    }
    /// Aligns this optical element with respect to another optical element.
    /// Specifically, the center (optical) axes of these to nodes are set on top of each other and the anchor points are separated by a given distance
    /// This helper function allows, e.g., to build a folded telescope (lens + 0° mirror) when the alignment beams propagate off-center through the lens.
    /// Remark: if this function is used, the distance specified at the `connect_nodes` function is ignored
    /// # Returns
    /// This function returns the original Node with updated alignment settings.
    #[must_use]
    fn align_like_node_at_distance(mut self, node_id: Uuid, distance: Length) -> Self {
        self.node_attr_mut()
            .set_align_like_node_at_distance(node_id, distance);
        self
    }
}

///trait to define an LIDT for a node
pub trait LIDT: OpticNode + Analyzable + Sized {
    /// Sets an LIDT value for all surfaces of this node
    ///
    /// # Errors
    ///
    /// This function returns an error if the given LIDT is negative or NaN.
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
        self.update_surfaces()?; // Wichtig: Damit die Runtime-Surfaces das Update mitbekommen
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
        node.set_isometry(iso)?;
        let local_trans = millimeter!(4.0, 5.0, 6.0);
        node.set_alignment(local_trans, degree!(0.0, 0.0, 0.0))?;
        let iso = node.effective_node_iso().ok_or(OpossumError::OpmDocument(
            "Error getting effective iso".to_string(),
        ))?;
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

        node.set_isometry(Isometry::identity())?;
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
