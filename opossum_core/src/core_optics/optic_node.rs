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
        NodeAttrExt, OpticNodeExt, OpticPorts, SurfacePorts,
        node_attr::{HasNodeAttr, NodePositioning},
        volumetric::Volumetric,
    },
    error::{OpmResult, OpossumError},
    geometry::{FaceIndex, Geometry},
    light::LightData,
    nodes::fluence_detector::Fluence,
    reporting::{Dottable, node_report::NodeReportResult},
    utils::geom_transformation::Isometry,
};

/// The report note of a detector whose apodization warning is set (see
/// [`OpticNode::set_apodization_warning`]): an optic shape limited what it measured.
pub(crate) const APERTURE_LIMIT_NOTE: &str =
    "An optic shape limits this measurement. Results might not be accurate.";

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

    /// Resets internal data: the captured light data, the caches and hit maps of the optic surfaces
    /// and the inversion in the body of the runtime geometry.
    fn reset_data(&mut self) {
        self.set_light_data(None);
        self.reset_optic_surfaces();
        if let Some(body) = self
            .node_attr_mut()
            .runtime_geometry_mut()
            .and_then(|geometry| geometry.kind_mut().as_body_mut())
        {
            *body.inversion_mut() = None;
        }
    }

    /// Prepare this node's medium before the first ray is traced (Phase A).
    ///
    /// Installs the node's surfaces anew from its **current** properties and placement, which also
    /// builds the body between them as the node's medium (see
    /// [`OpticNodeExt::install_geometry`](crate::core_optics::OpticNodeExt::install_geometry)), so
    /// [`Volumetric::propagate_inside_medium`] (Phase B) reads the very surfaces rays are traced at.
    /// If the operating point provides a gain model that reads the inversion, the
    /// [`InversionField`](crate::gain::InversionField) is built from the pump configuration and stored
    /// in that medium; otherwise the inversion stays `None`. Non-volume nodes return immediately,
    /// without installing surfaces or touching the medium.
    ///
    /// [`NodeGroup`](crate::nodes::NodeGroup) overrides this to recurse into every child node.
    ///
    /// # Errors
    ///
    /// Returns an error if the node's surfaces cannot be installed, if the node encloses no body, or
    /// if building the inversion field fails.
    fn prepare_volume(&mut self, strategy: &dyn PropagationStrategy) -> OpmResult<()> {
        if self.as_volume().is_none() {
            return Ok(());
        }
        self.update_surfaces()?;
        let config = strategy.pump_config(self.node_attr().uuid());
        let name = self.name().to_owned();
        let Some(body) = self
            .node_attr_mut()
            .runtime_geometry_mut()
            .and_then(|geometry| geometry.kind_mut().as_body_mut())
        else {
            return Err(OpossumError::Other(format!(
                "volume node '{name}' encloses no body"
            )));
        };
        // The inversion is gain-specific: only build it when the gain model reads one.
        if let Some(model) = config.gain_model().as_extraction() {
            let inversion = model.build_inversion(body, &config)?;
            *body.inversion_mut() = inversion;
        }
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

    /// Return the ports of this node, each with the face of its geometry it sits on.
    ///
    /// A node without a shape of its own has none ([`SurfacePorts::NONE`]).
    fn surface_ports(&self) -> SurfacePorts;

    /// Updates the optical surfaces of this node according to its current geometry and positioning:
    /// installs its geometry under its [`OpticNode::surface_ports`] (see
    /// [`OpticNodeExt::install_geometry`]).
    ///
    /// # Errors
    ///
    /// Returns an error if geometry evaluation or surface construction fails.
    fn update_surfaces(&mut self) -> OpmResult<()> {
        let ports = self.surface_ports();
        self.install_geometry(&ports)
    }

    /// Describe the shape of this node, derived from its current properties.
    ///
    /// The node type acts as a preset: a lens, for instance, describes itself as a
    /// [`Geometry::singlet`] whose parameters are its curvatures, center thickness and optic
    /// shape. The runtime surfaces rays are traced against are built from this description.
    ///
    /// # Returns
    ///
    /// The node's [`Geometry`], or `None` for a node without a shape of its own, such as a group
    /// or a reference to another node.
    ///
    /// # Errors
    ///
    /// Returns an error if a geometry property cannot be read or describes no valid shape.
    fn create_geometry(&self) -> OpmResult<Option<Geometry>>;

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
    /// Sets an LIDT value for all surfaces of this node.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given LIDT is negative or NaN,
    /// or if updating runtime surfaces fails.
    fn with_lidt(mut self, lidt: Fluence) -> OpmResult<Self> {
        if let Some(geometry) = self.create_geometry()? {
            for face in (0..geometry.kind().face_count()).map(FaceIndex::new) {
                self.set_lidt(face, lidt)?;
            }
        }
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
    /// A surface away from the node sits where its face does: the flat rear face of a lens placed at
    /// a turned node lies one center thickness behind it, along the node's axis.
    #[test]
    fn effective_surface_iso_of_a_face_behind_the_node() -> OpmResult<()> {
        use crate::{nodes::Lens, refractive_index::RefrIndexConst};
        let mut lens = Lens::new(
            "lens",
            millimeter!(50.0),
            millimeter!(f64::INFINITY),
            millimeter!(5.0),
            &RefrIndexConst::new(1.5)?,
        )?;
        let node_iso = Isometry::new(millimeter!(10.0, -20.0, 30.0), degree!(5.0, -10.0, 15.0))?;
        lens.set_positioning(NodePositioning::Absolute(node_iso))?;
        let expected = node_iso.append(&Isometry::new_along_z(millimeter!(5.0))?);
        let difference =
            expected.get_inv_transform() * lens.effective_surface_iso("output_1")?.get_transform();
        assert_abs_diff_eq!(difference.translation.vector.norm(), 0.0, epsilon = 1e-15);
        assert_abs_diff_eq!(difference.rotation.angle(), 0.0, epsilon = 1e-15);
        Ok(())
    }
}
