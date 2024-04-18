//! Common optcial node attributes.
//!
//! This module handles common attributes of optical nodes such as [`Properties`] or geometric data (isometries, etc.)
use serde::{Deserialize, Serialize};

use crate::{
    error::{OpmResult, OpossumError},
    optic_ports::OpticPorts,
    properties::{PropCondition, Properties, Proptype},
    utils::geom_transformation::Isometry,
};

/// Struct for sotring common attributes of optical nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAttr {
    node_type: String,
    props: Properties,
    isometry: Isometry,
}
impl NodeAttr {
    /// Creates new node attributes ([`NodeAttr`]).
    ///
    /// This automatically creates some "standard" properties common to all optic nodes (name, node type, inverted, apertures)
    /// # Panics
    ///
    /// Panics theoretically if the standarnd properties could not be created.
    #[must_use]
    pub fn new(name: &str, node_type: &str) -> Self {
        let mut properties = Properties::default();
        properties
            .create(
                "name",
                "name of the optical element",
                Some(vec![PropCondition::NonEmptyString]),
                name.into(),
            )
            .unwrap();
        properties
            .create("inverted", "inverse propagation?", None, false.into())
            .unwrap();
        properties
            .create(
                "apertures",
                "input and output apertures of the optical element",
                None,
                OpticPorts::default().into(),
            )
            .unwrap();
        Self {
            node_type: node_type.into(),
            props: properties,
            isometry: Isometry::default(),
        }
    }
    /// Returns the name property of this node.
    ///
    /// # Errors
    ///
    /// This function will return an error if the property `name` and the property `node_type` does not exist.
    #[must_use]
    pub fn name(&self) -> String {
        if let Ok(Proptype::String(name)) = &self.props.get("name") {
            name.into()
        } else {
            self.node_type()
        }
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
    pub fn inverted(&self) -> OpmResult<bool> {
        self.props.get_bool("inverted")
    }
    /// Sets a property of this [`NodeAttr`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the property does not exist or has the wrong [`Proptype`].
    pub fn set_property(&mut self, name: &str, value: Proptype) -> OpmResult<()> {
        self.props.set(name, value)
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
        conditions: Option<Vec<PropCondition>>,
        value: Proptype,
    ) -> OpmResult<()> {
        self.props.create(name, description, conditions, value)
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
    pub fn set_isometry(&mut self, isometry: Isometry) {
        self.isometry = isometry;
    }
    /// Returns a reference to the isometry of this [`NodeAttr`].
    #[must_use]
    pub const fn isometry(&self) -> &Isometry {
        &self.isometry
    }
}
