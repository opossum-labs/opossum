//! Common optical node attributes.
//!
//! This module provides common attributes and utilities for optical nodes, such as [`Properties`], geometric data (isometries), and GUI positioning.
//! These attributes are shared across different types of optical nodes in the system.
use nalgebra::Point2;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uom::si::f64::Length;
use uuid::Uuid;

use super::fluence_detector::Fluence;
use crate::{
    J_per_cm2,
    error::{OpmResult, OpossumError},
    optic_ports::OpticPorts,
    optic_scenery_rsc::SceneryResources,
    properties::{Properties, Proptype},
    utils::geom_transformation::Isometry,
};

/// Struct for storing common attributes of optical nodes.
///
/// `NodeAttr` encapsulates metadata and configuration for an optical node, including its type, name, ports, unique identifier,
/// laser-induced damage threshold (LIDT), geometric transformations, alignment, and frontend GUI position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAttr {
    /// The type of the node (e.g., "lens", "mirror").
    node_type: String,
    /// The name of the node.
    name: String,
    ports: OpticPorts,
    /// Universally unique identifier for this node.
    uuid: Uuid,
    lidt: Fluence,
    #[serde(default)]
    props: Properties,
    #[serde(skip_serializing_if = "Option::is_none")]
    isometry: Option<Isometry>,
    #[serde(default)]
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
            global_conf: None,
            isometry: None,
            inverted: false,
            alignment: None,
            align_like_node_at_distance: None,
            uuid: Uuid::new_v4(),
            lidt: J_per_cm2!(1.),
            gui_position: None,
        }
    }
    /// Returns the name property of this node.
    ///
    /// # Errors
    ///
    /// This function will return an error if the property `name` and the property `node_type` does not exist.
    #[must_use]
    pub fn name(&self) -> String {
        self.name.clone()
    }
    /// Returns the node-type property of this node.
    ///
    /// # Errors
    ///
    /// This function will return an error if the property `node_type` does not exist.
    #[must_use]
    pub fn node_type(&self) -> String {
        self.node_type.clone()
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
    /// Returns a reference to the isometry of this [`NodeAttr`].
    #[must_use]
    pub fn isometry(&self) -> Option<Isometry> {
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
        self.name = name.to_string();
    }
    /// Sets this [`NodeAttr`] as `inverted`.
    pub const fn set_inverted(&mut self, inverted: bool) {
        self.inverted = inverted;
    }
    /// Returns a reference to the optic ports of this [`NodeAttr`].
    #[must_use]
    pub const fn ports(&self) -> &OpticPorts {
        &self.ports
    }

    /// Returns a mutable reference to the optic ports of this [`NodeAttr`].
    #[must_use]
    pub const fn ports_mut(&mut self) -> &mut OpticPorts {
        &mut self.ports
    }
    /// Sets the apertures of this [`NodeAttr`].
    pub fn set_ports(&mut self, ports: OpticPorts) {
        self.ports = ports;
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

    /// Returns a reference to the lidt of this [`NodeAttr`].
    #[must_use]
    pub const fn lidt(&self) -> &Fluence {
        &self.lidt
    }
    ///Sets the lidt of this [`NodeAttr`].
    pub fn set_lidt(&mut self, lidt: &Fluence) {
        self.lidt = *lidt;
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
}
