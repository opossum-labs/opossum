#![warn(missing_docs)]
//! Contains the basic trait representing an optical element
#[cfg(feature = "bevy")]
use bevy::{math::primitives::Cuboid, render::mesh::Mesh};
use log::warn;
use nalgebra::{Point3, Vector3};
use petgraph::stable_graph::NodeIndex;
use uom::si::f64::{Angle, Length};

use crate::aperture::Aperture;
use crate::coatings::CoatingType;
use crate::dottable::Dottable;
use crate::error::{OpmResult, OpossumError};
use crate::lightdata::LightData;
use crate::nodes::{NodeAttr, NodeGroup, NodeReference};
use crate::optic_ports::{OpticPorts, PortType};
use crate::optic_senery_rsc::SceneryResources;
use crate::properties::{Properties, Proptype};
use crate::refractive_index::RefractiveIndexType;
use crate::reporter::NodeReport;
use crate::utils::geom_transformation::Isometry;
use core::fmt::Debug;
use std::cell::RefCell;
use std::fmt::Display;
use std::path::Path;
use std::rc::Rc;

/// This is the basic trait that must be implemented by all concrete optical components.
pub trait OpticNode: Dottable {
    /// Return the available (input & output) ports of this [`OpticNode`].
    fn ports(&self) -> OpticPorts {
        let mut ports = self.node_attr().ports().clone();
        if self.node_attr().inverted() {
            ports.set_inverted(true);
        }
        ports
    }
    /// Set an [`Aperture`] for a given port name.
    ///
    /// # Errors
    /// This function will return an error if the port name does not exist.
    fn set_aperture(
        &mut self,
        port_type: &PortType,
        port_name: &str,
        aperture: &Aperture,
    ) -> OpmResult<()> {
        let mut ports = self.ports();
        ports.set_aperture(port_type, port_name, aperture)?;
        self.node_attr_mut().set_ports(ports);
        Ok(())
    }
    /// Set a coating for a given port name.
    ///
    /// # Errors
    ///
    /// This function will return an error if the port name does not exist.
    fn set_coating(
        &mut self,
        port_type: &PortType,
        port_name: &str,
        coating: &CoatingType,
    ) -> OpmResult<()> {
        let mut ports = self.ports();
        ports.set_coating(port_type, port_name, coating)?;
        self.node_attr_mut().set_ports(ports);
        Ok(())
    }
    /// define the up-direction of this lightdata's first ray which is needed to create an isometry from this ray.
    /// This function should only be used during the node positioning process, and only for source nodes.
    ///
    /// # Errors
    /// This function errors if the the lightdata is not geometric
    fn define_up_direction(&self, ray_data: &LightData) -> OpmResult<Vector3<f64>> {
        if let LightData::Geometric(rays) = ray_data {
            rays.define_up_direction()
        } else {
            Err(OpossumError::Other(
                "Wrong light data for \"up-direction\" definition".into(),
            ))
        }
    }
    /// Modifies the current up-direction of a ray, stored in lightdata, which is needed to create an isometry from this ray.
    /// This function should only be used during the node positioning process.
    ///
    /// # Errors
    /// This function errors if the the lightdata is not geometric
    fn calc_new_up_direction(
        &self,
        ray_data: &LightData,
        up_direction: &mut Vector3<f64>,
    ) -> OpmResult<()> {
        if let LightData::Geometric(rays) = ray_data {
            rays.calc_new_up_direction(up_direction)?;
        } else {
            return Err(OpossumError::Other(
                "Wrong light data for \"up-direction\" calculation".into(),
            ));
        }
        Ok(())
    }
    /// Export analysis data to file(s) within the given directory path.
    ///
    /// This function should be overridden by a node in order to export node-specific data into a file.
    /// The default implementation does nothing.
    ///
    /// # Errors
    /// This function might return an error depending on the particular implementation.
    fn export_data(&self, _data_dir: &Path, _uuid: &str) -> OpmResult<()> {
        Ok(())
    }
    /// Returns `true` if the [`OpticNode`] represents a detector that can report analysis data.
    fn is_detector(&self) -> bool {
        false
    }
    /// Return a downcasted reference of a [`NodeGroup`].
    ///
    /// # Errors
    /// This function will return an error if the [`OpticNode`] does not have the `node_type` property "group".
    fn as_group(&mut self) -> OpmResult<&mut NodeGroup> {
        Err(OpossumError::Other("cannot cast to group".into()))
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
        Ok(())
    }
    /// Return a downcasted mutable reference of a [`NodeReference`].
    ///
    /// # Errors
    /// This function will return an error if the [`OpticNode`] does not have the `node_type` property "reference".
    fn as_refnode_mut(&mut self) -> OpmResult<&mut NodeReference> {
        Err(OpossumError::Other("cannot cast to reference node".into()))
    }
    /// Set a property of this [`OpticNode`].
    ///
    /// Set a property of an optical node. This property must already exist (e.g. defined in `new()` / `default()` functions of the node).
    ///
    /// # Errors
    /// This function will return an error if a non-defined property is set or the property has the wrong data type.
    fn set_property(&mut self, name: &str, proptype: Proptype) -> OpmResult<()> {
        self.node_attr_mut().set_property(name, proptype)
    }
    /// Set all properties of this [`OpticNode`].
    ///
    /// This is a convenience function. It internally calls [`set_property`](OpticNode::set_property) for all given properties. **Note**: Properties, which are not
    /// present for the [`OpticNode`] are silently ignored.
    ///
    /// # Errors
    /// This function will return an error if the Property conditions while setting a value are not met.
    fn set_properties(&mut self, properties: Properties) -> OpmResult<()> {
        let own_properties = self.properties().clone();
        for prop in &properties {
            if own_properties.contains(prop.0) {
                match prop.0.as_str() {
                    "node_type" => {}
                    "apertures" => {
                        let mut ports = self.ports();
                        if let Proptype::OpticPorts(ports_to_be_set) = prop.1.prop().clone() {
                            if self.node_type() == "group" {
                                // apertures cannot be set here for groups since no port mapping is defined yet.
                                // this will be done later dynamically in group:ports() function.
                                self.node_attr_mut().set_ports(ports_to_be_set);
                            } else {
                                ports.set_apertures(ports_to_be_set)?;
                                self.node_attr_mut().set_ports(ports);
                            }
                        }
                    }
                    _ => self.set_property(prop.0, prop.1.prop().clone())?,
                };
            }
        }
        Ok(())
    }
    /// Set this [`OpticNode`] as inverted.
    ///
    /// This flag signifies that the [`OpticNode`] should be propagated in reverse order. This function normally simply sets the
    /// `inverted` property. For [`NodeGroup`] it also sets the `inverted` flag of the underlying `OpticGraph`.
    ///
    /// # Errors
    /// This function returns an error, if the node cannot be inverted. This is the case, if
    ///   - it is a source node
    ///   - it is a group node containing a non-invertable node (e.g. a source)
    fn set_inverted(&mut self, inverted: bool) -> OpmResult<()> {
        self.node_attr_mut().set_inverted(inverted);
        Ok(())
    }
    /// Returns `true` if the node should be analyzed in reverse direction.
    fn inverted(&self) -> bool {
        self.node_attr().inverted()
    }
    /// Return a YAML representation of the current state of this [`OpticNode`].
    ///
    /// This function must be overridden for generating output in the analysis report. Mainly
    /// detector nodes use this feature.
    fn report(&self, _uuid: &str) -> Option<NodeReport> {
        None
    }
    /// Get the [`NodeAttr`] (common attributes) of an [`OpticNode`].
    fn node_attr(&self) -> &NodeAttr;
    /// Get the mutable[`NodeAttr`] (common attributes) of an [`OpticNode`].
    fn node_attr_mut(&mut self) -> &mut NodeAttr;
    /// Update node attributes of this [`OpticNode`] from given [`NodeAttr`].
    fn set_node_attr(&mut self, node_attributes: NodeAttr) {
        let node_attr_mut = self.node_attr_mut();
        if let Some(iso) = node_attributes.isometry() {
            node_attr_mut.set_isometry(iso);
        }
        if let Some(alignment) = node_attributes.alignment() {
            node_attr_mut.set_alignment(alignment.clone());
        }
        node_attr_mut.set_name(&node_attributes.name());
        node_attr_mut.set_inverted(node_attributes.inverted());
        if let Some((node_idx, distance)) = node_attributes.get_align_like_node_at_distance() {
            node_attr_mut.set_align_like_node_at_distance(*node_idx, *distance);
        }
        node_attr_mut.update_properties(node_attributes.properties().clone());
        node_attr_mut.set_ports(node_attributes.ports().clone());
    }
    /// Get the node type of this [`OpticNode`]
    fn node_type(&self) -> String {
        self.node_attr().node_type()
    }
    /// Get the name of this [`OpticNode`]
    fn name(&self) -> String {
        self.node_attr().name()
    }
    /// Return all properties of this [`OpticNode`].
    fn properties(&self) -> &Properties {
        self.node_attr().properties()
    }
    /// Return the (base) [`Isometry`] of this optical node.
    fn isometry(&self) -> Option<Isometry> {
        self.node_attr().isometry()
    }
    /// Set the (base) [`Isometry`] (position and angle) of this optical node.
    fn set_isometry(&mut self, isometry: Isometry) {
        self.node_attr_mut().set_isometry(isometry);
    }
    /// Return the effective input isometry of this optical node.
    ///
    /// The effective input isometry is the base isometry modified by the local alignment isometry (if any).
    fn effective_iso(&self) -> Option<Isometry> {
        self.isometry().as_ref().and_then(|iso| {
            self.node_attr().alignment().as_ref().map_or_else(
                || Some(iso.clone()),
                |local_iso| Some(iso.append(local_iso)),
            )
        })
    }
    /// Set local alignment (decenter, tilt) of an optical node.
    ///
    /// # Errors
    ///
    /// This function will return an error if the `alignment` property cannot be set.
    fn set_alignment(&mut self, decenter: Point3<Length>, tilt: Point3<Angle>) -> OpmResult<()> {
        let align = Some(Isometry::new(decenter, tilt)?);
        self.node_attr_mut()
            .set_property("alignment", align.into())?;
        Ok(())
    }
    ///
    #[cfg(feature = "bevy")]
    fn mesh(&self) -> Mesh {
        let mesh: Mesh = Cuboid::new(0.3, 0.3, 0.001).into();
        if let Some(iso) = self.effective_iso() {
            mesh.transformed_by(iso.into())
        } else {
            warn!("Node has no isometry defined. Mesh will be located at origin.");
            mesh
        }
    }
    /// Get a refrecne to a global configuration (if any).
    fn global_conf(&self) -> &Option<Rc<RefCell<SceneryResources>>> {
        self.node_attr().global_conf()
    }
    /// Set the global configuration for this [`OpticNode`].
    /// **Note**: This function should normally only be used by [`OpticRef`](crate::optic_ref::OpticRef).
    fn set_global_conf(&mut self, global_conf: Option<Rc<RefCell<SceneryResources>>>) {
        let node_attr = self.node_attr_mut();
        node_attr.set_global_conf(global_conf);
    }
    /// Get the ambient refractive index.
    ///
    /// This value is determined by the global configuration. A warning is issued and a default value is returned
    /// if the global config could not be found.
    fn ambient_idx(&self) -> RefractiveIndexType {
        self.global_conf().as_ref().map_or_else(
            || {
                warn!(
                    "could not get ambient medium since global config not found ... using default"
                );
                SceneryResources::default().ambient_refr_index
            },
            |conf| conf.borrow().ambient_refr_index.clone(),
        )
    }
}
impl Debug for dyn OpticNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}' ({})", self.name(), self.node_type())
    }
}
impl Display for dyn OpticNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}' ({})", self.name(), self.node_type())
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
    fn align_like_node_at_distance(mut self, node_idx: NodeIndex, distance: Length) -> Self {
        self.node_attr_mut()
            .set_align_like_node_at_distance(node_idx, distance);
        self
    }
}
