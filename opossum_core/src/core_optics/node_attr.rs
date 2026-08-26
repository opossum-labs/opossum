//! Common optical node attributes.
//!
//! This module provides common attributes and utilities for optical nodes, such as [`Properties`], geometric data (isometries), and GUI positioning.
//! These attributes are shared across different types of optical nodes in the system.
use nalgebra::Point2;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use uom::si::f64::Length;
use uuid::Uuid;

use crate::{
    core_optics::{OpticPorts, SceneryResources, optic_surface::OpticSurface},
    error::{OpmResult, OpossumError},
    gain::InversionField,
    geometry::body::SurfaceBoundedBody,
    properties::{Properties, Proptype, validator::Validator},
    utils::{file_utils::sanitize_filename, geom_transformation::Isometry},
};

/// Container for runtime state of an optical node
#[derive(Default, Debug, Clone)]
pub struct RuntimeSurfaces {
    pub inputs: BTreeMap<String, OpticSurface>,
    pub outputs: BTreeMap<String, OpticSurface>,
}
impl RuntimeSurfaces {
    /// Returns an iterator over all mutable optic surfaces (inputs and outputs).
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut OpticSurface> {
        self.inputs.values_mut().chain(self.outputs.values_mut())
    }

    /// Returns an iterator over all optic surfaces (inputs and outputs) and their port names.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &OpticSurface)> {
        self.inputs.iter().chain(self.outputs.iter())
    }
}
/// The volume state a node was prepared with for the current analysis run.
///
/// The counterpart of [`RuntimeSurfaces`] for what lies *between* the surfaces. Built once per node
/// per run by [`OpticNode::prepare_volume`](crate::core_optics::OpticNode::prepare_volume) and
/// cleared by [`OpticNode::reset_data`](crate::core_optics::OpticNode::reset_data).
#[derive(Debug, Clone)]
pub struct RuntimeMedium {
    body: SurfaceBoundedBody,
    inversion: Option<InversionField>,
}
impl RuntimeMedium {
    /// Return the volume body this node was prepared with.
    #[must_use]
    pub fn body(&self) -> &dyn crate::geometry::body::Body {
        &self.body
    }
    /// Return the inversion field, if the model built one.
    #[must_use]
    pub fn inversion(&self) -> Option<&InversionField> {
        self.inversion.as_ref()
    }
    /// Set the inversion field of this [`RuntimeMedium`]
    pub fn set_inversion(&mut self, inversion: Option<InversionField>){
        self.inversion = inversion;
    }
}

fn deserialize_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Ok(sanitize_filename(&s))
}

/// Struct for storing common attributes of optical nodes.
///
/// `NodeAttr` encapsulates metadata and configuration for an optical node, including its type, name, ports, unique identifier,
/// laser-induced damage threshold (LIDT), geometric transformations, alignment, and frontend GUI position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAttr {
    /// The type of the node (e.g., "lens", "mirror").
    node_type: String,
    /// The name of the node.
    #[serde(deserialize_with = "deserialize_name")]
    name: String,
    #[serde(default, skip_serializing_if = "OpticPorts::is_all_default")]
    ports: OpticPorts,
    #[serde(skip)]
    runtime_surfaces: RuntimeSurfaces,
    #[serde(skip)]
    runtime_medium: Option<RuntimeMedium>,
    uuid: Uuid,
    #[serde(default, skip_serializing_if = "Properties::is_empty")]
    props: Properties,
    #[serde(skip_serializing_if = "Option::is_none")]
    isometry: Option<Isometry>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    inverted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    alignment: Option<Isometry>,
    #[serde(skip)]
    global_conf: Option<Arc<Mutex<SceneryResources>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    align_like_node_at_distance: Option<(Uuid, Length)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gui_position: Option<Point2<f64>>,
}
impl NodeAttr {
    /// Creates new node attributes ([`NodeAttr`]).
    ///
    /// This constructor initializes a node with standard default properties common to all optical nodes:
    /// - `name`: Set to the provided `node_type` string.
    /// - `node_type`: Set to the provided `node_type` string.
    /// - `inverted`: Set to `false`.
    /// - `ports`: Set to default (empty) [`OpticPorts`] structure.
    /// - `alignment`: Set to `None`.
    /// - `uuid`: Randomly generated unique identifier.
    /// - `lidt`: Set to a default fluence value of 1 J/cm².
    /// - `gui_position`: Set to `None`.
    ///
    /// # Arguments
    ///
    /// * `node_type` - The type of the optical node (e.g., "lens", "mirror").
    ///
    /// # Panics
    ///
    /// This function may theoretically panic if the standard properties could not be created,
    /// but this should not occur under normal circumstances.
    #[must_use]
    pub fn new(node_type: &str) -> Self {
        Self {
            node_type: node_type.into(),
            name: node_type.into(),
            props: Properties::default(),
            ports: OpticPorts::default(),
            runtime_surfaces: RuntimeSurfaces::default(),
            runtime_medium: None,
            global_conf: None,
            isometry: None,
            inverted: false,
            alignment: None,
            align_like_node_at_distance: None,
            uuid: Uuid::new_v4(),
            gui_position: None,
        }
    }
    /// Returns the name property of this node.
    ///
    /// # Errors
    ///
    /// This function will return an error if the property `name` and the property `node_type` does not exist.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns the node-type property of this node.
    ///
    /// # Errors
    ///
    /// This function will return an error if the property `node_type` does not exist.
    #[must_use]
    pub fn node_type(&self) -> &str {
        &self.node_type
    }
    /// Returns the inversion property of thie node.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying property `inverted` does not exist or has the wrong datatype.
    #[must_use]
    pub const fn inverted(&self) -> bool {
        self.inverted
    }
    /// Sets a property of this [`NodeAttr`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the property does not exist or has the wrong [`Proptype`].
    pub fn set_property(&mut self, name: &str, value: Proptype) -> OpmResult<()> {
        self.props.set(name, value)
    }
    /// Update the [`Properties`] section of this [`NodeAttr`].
    pub fn update_properties(&mut self, new_props: Properties) {
        self.props.update(new_props);
    }
    /// Create a property within this [`NodeAttr`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the property already exists.
    pub fn create_property(
        &mut self,
        name: &str,
        description: &str,
        value: Proptype,
    ) -> OpmResult<()> {
        self.props.create(name, description, value)
    }
    /// Create a property (with validator) within this [`NodeAttr`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the property already exists or the validation fails with the given initial value.
    pub fn create_property_with_validator(
        &mut self,
        name: &str,
        description: &str,
        // validator: Box<dyn Validator>,
        validator: Validator,
        value: Proptype,
    ) -> OpmResult<()> {
        self.props
            .create_with_validator(name, description, validator, value)
    }
    /// Returns a reference to the properties of this [`NodeAttr`].
    #[must_use]
    pub const fn properties(&self) -> &Properties {
        &self.props
    }
    /// Return a propery value [`Proptype`] for this [`NodeAttr`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the property with the given name was not found.
    pub fn get_property(&self, name: &str) -> OpmResult<&Proptype> {
        self.props.get(name)
    }

    /// Return the value of a boolean property.
    ///
    /// # Errors
    ///
    /// This function will return an error if the property with the given name does not exist or is not a
    /// boolean property.
    pub fn get_property_bool(&self, name: &str) -> OpmResult<bool> {
        let bool_prop = self.props.get(name)?;
        if let Proptype::Bool(value) = bool_prop {
            Ok(*value)
        } else {
            Err(OpossumError::Other("not a bool property".into()))
        }
    }
    /// Sets the isometry of this [`NodeAttr`].
    pub const fn set_isometry(&mut self, isometry: Isometry) {
        self.isometry = Some(isometry);
    }

    /// Sets the isometry option of this [`NodeAttr`].
    pub const fn set_isometry_option(&mut self, isometry_opt: Option<Isometry>) {
        self.isometry = isometry_opt;
    }
    /// Returns a reference to the isometry of this [`NodeAttr`].
    #[must_use]
    pub const fn isometry(&self) -> Option<Isometry> {
        self.isometry
    }
    /// Returns the local alignment isometry of a node (if any).
    #[must_use]
    pub const fn alignment(&self) -> &Option<Isometry> {
        &self.alignment
    }
    /// Sets the local alignment isometry of this [`NodeAttr`].
    ///
    /// # Panics
    /// This function could theoretically panic if the property `alignment` is not defined.
    pub const fn set_alignment(&mut self, isometry: Isometry) {
        self.alignment = Some(isometry);
    }
    /// Sets the local alignment isometry option of this [`NodeAttr`], allowing it to be cleared back to
    /// unset (unlike [`Self::set_alignment`], which can only ever set a concrete value).
    pub const fn set_alignment_option(&mut self, alignment_opt: Option<Isometry>) {
        self.alignment = alignment_opt;
    }
    /// Returns a reference to the global config (if any) of this [`NodeAttr`].
    #[must_use]
    pub const fn global_conf(&self) -> &Option<Arc<Mutex<SceneryResources>>> {
        &self.global_conf
    }
    /// Sets the global conf of this [`NodeAttr`].
    pub fn set_global_conf(&mut self, global_conf: Option<Arc<Mutex<SceneryResources>>>) {
        self.global_conf = global_conf;
    }
    /// Sets the name of this [`NodeAttr`].
    pub fn set_name(&mut self, name: &str) {
        self.name = sanitize_filename(name);
    }
    /// Sets this [`NodeAttr`] as `inverted`.
    pub const fn set_inverted(&mut self, inverted: bool) {
        self.inverted = inverted;
    }
    /// Returns a reference to the stored optic ports of this [`NodeAttr`].
    ///
    /// **Warning**: This method only returns the internally stored port configuration.
    /// For virtual nodes like [`NodeReference`](crate::nodes::NodeReference), this might be empty. To get the
    /// effective ports of an optical element, always use the `ports()` method
    /// of the [`OpticNode`](crate::core_optics::OpticNode) trait instead!
    #[must_use]
    pub const fn raw_ports(&self) -> &OpticPorts {
        &self.ports
    }

    /// Returns a mutable reference to the optic ports of this [`NodeAttr`].
    ///
    /// **Warning**: See [`raw_ports()`](NodeAttr::raw_ports). Use
    /// the [`OpticNode`](crate::core_optics::OpticNode) trait methods for
    /// safe modifications of effective ports.
    #[must_use]
    pub const fn raw_ports_mut(&mut self) -> &mut OpticPorts {
        &mut self.ports
    }
    /// Sets the apertures of this [`NodeAttr`].
    pub fn set_ports(&mut self, ports: OpticPorts) {
        self.ports = ports;
    }
    /// Returns a mutable reference to the runtime surfaces.
    pub const fn runtime_surfaces_mut(&mut self) -> &mut RuntimeSurfaces {
        &mut self.runtime_surfaces
    }

    /// Returns a reference to the runtime surfaces.
    #[must_use]
    pub const fn runtime_surfaces(&self) -> &RuntimeSurfaces {
        &self.runtime_surfaces
    }
    /// Return the prepared medium for this node, if any.
    ///
    /// Set by [`OpticNode::prepare_volume`](crate::core_optics::OpticNode::prepare_volume), cleared
    /// by [`OpticNode::reset_data`](crate::core_optics::OpticNode::reset_data). `None` means the
    /// node has not been prepared yet or has been reset.
    #[must_use]
    pub const fn runtime_medium(&self) -> Option<&RuntimeMedium> {
        self.runtime_medium.as_ref()
    }
    /// Store the prepared medium for this node.
    ///
    /// # Arguments
    ///
    /// * `body` - the volume the light passes through.
    /// * `inversion` - the inversion field the model built, or `None` if it built none.
    pub fn set_runtime_medium(
        &mut self,
        body: SurfaceBoundedBody,
        inversion: Option<InversionField>,
    ) {
        self.runtime_medium = Some(RuntimeMedium { body, inversion });
    }
    pub fn set_runtime_inversion(
        &mut self,
        inversion: Option<InversionField>,
    ) {
        self.runtime_medium.as_mut().map(|medium| medium.set_inversion(inversion));
    }
    /// Clear the prepared medium for this node.
    ///
    /// Called by [`OpticNode::reset_data`](crate::core_optics::OpticNode::reset_data) to discard the
    /// state between analysis runs.
    pub fn clear_runtime_inversion(&mut self) {
        if let Some(medium) = self.runtime_medium.as_mut(){
            medium.set_inversion(None);
        }
    }
    /// Returns a reference to the uuid of this [`NodeAttr`].
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.uuid
    }
    ///Sets the uuid of this [`NodeAttr`].
    pub const fn set_uuid(&mut self, uuid: Uuid) {
        self.uuid = uuid;
    }

    /// set the nodeindex and distance of the node to which this node should be aligned to
    pub fn set_align_like_node_at_distance(&mut self, node_id: Uuid, distance: Length) {
        self.align_like_node_at_distance = Some((node_id, distance));
    }

    /// get the nodeindex and distance of the node to which this node should be aligned to
    #[must_use]
    pub const fn get_align_like_node_at_distance(&self) -> &Option<(Uuid, Length)> {
        &self.align_like_node_at_distance
    }
    /// Returns the GUI position of this optical node.
    ///
    /// This function returns the position of the node in a frontend diagram, if set.
    /// If the value is `None`, the node may be placed automatically by the frontend.
    ///
    /// The position is a [`Point2`] since the `x` & `y` coordinates represent the position on a 2D
    /// frontend diagram.
    #[must_use]
    pub const fn gui_position(&self) -> Option<Point2<f64>> {
        self.gui_position
    }
    /// Sets the GUI position of this optical node.
    pub const fn set_gui_position(&mut self, gui_position: Option<Point2<f64>>) {
        self.gui_position = gui_position;
    }

    /// Replaces itself with a copy of the passed [`NodeAttr`] but keeps its original uuid
    pub fn replace_from_node_attr(&mut self, node_attr: &Self) {
        let id = self.uuid;
        // Beim Ersetzen kopieren wir auch die Config. Die Runtime Surfaces
        // sollten durch update_surfaces() des Nodes neu aufgebaut werden.
        *self = node_attr.clone();
        self.uuid = id;
    }
}

/// Trait for basic optical node attribute access.
pub trait HasNodeAttr {
    fn node_attr(&self) -> &NodeAttr;
    fn node_attr_mut(&mut self) -> &mut NodeAttr;
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Aperture, ApertureType},
        geometry::{Plane, body::SurfaceBoundedBody, geo_surface::GeoSurfaceRef},
        millimeter,
        types::validated_type_definitions::ValidatedCrossSection,
    };
    use std::sync::{Arc, Mutex};

    fn test_body() -> OpmResult<SurfaceBoundedBody> {
        Ok(SurfaceBoundedBody::new(
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(Isometry::identity())))),
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(Isometry::new_along_z(
                millimeter!(10.0),
            )?)))),
            ValidatedCrossSection::try_new(Aperture::new_circle(
                millimeter!(5.0),
                ApertureType::Hole,
                None,
            )?)?,
            Isometry::identity(),
        ))
    }

    #[test]
    fn runtime_medium_starts_unset() {
        assert!(NodeAttr::new("test").runtime_medium().is_none());
    }
    #[test]
    fn set_and_clear_runtime_inversion() -> OpmResult<()> {
        let mut attr = NodeAttr::new("test");
        attr.set_runtime_medium(test_body()?, None);
        assert!(attr.runtime_medium().is_some());
        attr.clear_runtime_inversion();
        assert!(attr.runtime_medium().is_some());
        assert!(attr.runtime_medium().unwrap().inversion().is_none());
        Ok(())
    }
    #[test]
    fn runtime_medium_is_not_in_ron_roundtrip() -> OpmResult<()> {
        let mut attr = NodeAttr::new("test");
        attr.set_runtime_medium(test_body()?, None);
        let serialized = ron::to_string(&attr).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert!(
            !serialized.contains("runtime_medium"),
            "runtime_medium must not be serialized: {serialized}"
        );
        let back: NodeAttr =
            ron::from_str(&serialized).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert!(back.runtime_medium().is_none());
        Ok(())
    }

    #[test]
    fn set_name_sanitization() {
        let mut attr = NodeAttr::new("test");
        attr.set_name("../bad_name");
        assert_eq!(attr.name(), ".._bad_name");
    }

    #[test]
    fn deserialize_name_sanitization() -> OpmResult<()> {
        let ron_str = r#"
            (
                node_type: "test",
                name: "../malicious",
                uuid: "98248e7f-dc4c-4131-8710-f3d5be2ff087",
                ports: (
                    inputs: {},
                    outputs: {}
                ),
                lidt: 1.0
            )
        "#;
        let attr: NodeAttr =
            ron::from_str(ron_str).map_err(|e| OpossumError::OpmDocument(e.to_string()))?;
        assert_eq!(attr.name(), ".._malicious");
        Ok(())
    }
}
