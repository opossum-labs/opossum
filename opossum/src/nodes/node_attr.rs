//! Common optcial node attributes.
//!
//! This module handles common attributes of optical nodes such as [`Properties`] or geometric data (isometries, etc.)
use std::{cell::RefCell, rc::Rc};

use serde::{Deserialize, Serialize};

use crate::{
    error::{OpmResult, OpossumError},
    optic_ports::OpticPorts,
    optic_senery_rsc::SceneryResources,
    properties::{PropCondition, Properties, Proptype},
    utils::{geom_transformation::Isometry, EnumProxy},
};

/// Struct for sotring common attributes of optical nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAttr {
    #[serde(skip)]
    node_type: String,
    props: Properties,
    isometry: Option<Isometry>,
    #[serde(skip)]
    global_conf: Option<Rc<RefCell<SceneryResources>>>,
}
impl NodeAttr {
    /// Creates new node attributes ([`NodeAttr`]).
    ///
    /// This automatically creates some "standard" properties common to all optic nodes (name, node type, inverted, apertures). The
    /// standard properties / values are:
    ///   - `name`: the given `node_type`
    ///   - `inverted`: `false`
    ///   - `apertures`: default [`OpticPorts`] structure
    ///   - `alignment`: `None`
    /// # Panics
    ///
    /// Panics theoretically if the standarnd properties could not be created.
    #[must_use]
    pub fn new(node_type: &str) -> Self {
        let mut properties = Properties::default();
        properties
            .create(
                "name",
                "name of the optical element",
                Some(vec![PropCondition::NonEmptyString]),
                node_type.into(),
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
        properties
            .create(
                "plot_aperture",
                "flag that defines if the aperture is displayed in a plot",
                None,
                false.into(),
            )
            .unwrap();
        properties
            .create(
                "alignment",
                "local alignment (decenter, tilt) of the optical element",
                None,
                EnumProxy::<Option<Isometry>> { value: None }.into(),
            )
            .unwrap();

        Self {
            node_type: node_type.into(),
            props: properties,
            global_conf: None,
            isometry: None,
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
        self.isometry = Some(isometry);
    }
    /// Returns a reference to the isometry of this [`NodeAttr`].
    #[must_use]
    pub fn isometry(&self) -> Option<Isometry> {
        self.isometry.clone()
    }
    /// Returns the local alignment isometry of a node (if any).
    #[must_use]
    pub fn alignment(&self) -> &Option<Isometry> {
        if let Ok(Proptype::Isometry(prox)) = self.props.get("alignment") {
            &prox.value
        } else {
            &None
        }
    }
    /// Sets the local alignment isometry of this [`NodeAttr`].
    ///
    /// # Panics
    /// This function could theoretically panic if the property `alignment` is not defined.
    pub fn set_alignment(&mut self, isometry: Isometry) {
        self.props.set("alignment", Some(isometry).into()).unwrap();
    }
    /// Returns a reference to the global config (if any) of this [`NodeAttr`].
    #[must_use]
    pub const fn global_conf(&self) -> &Option<Rc<RefCell<SceneryResources>>> {
        &self.global_conf
    }
    /// Sets the global conf of this [`NodeAttr`].
    pub fn set_global_conf(&mut self, global_conf: Option<Rc<RefCell<SceneryResources>>>) {
        self.global_conf = global_conf;
    }
}
