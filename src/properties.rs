//! Module for handling node properties
use plotters::prelude::LogScalable;
use serde_derive::{Deserialize, Serialize};
use std::{collections::HashMap, mem};
use uuid::Uuid;

use crate::{
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    nodes::{FilterType, Metertype, PortMap, SpectrometerType},
    optic_graph::OpticGraph,
};
/// A general set of (optical) properties.
///
/// The property system is used for storing node specific parameters (such as focal length, splitting ratio, filter curve, etc ...).
/// Properties have to be created once before they can be set and used.
///
/// ## Example
/// ```rust
/// use opossum::properties::Properties;
/// let mut props = Properties::default();
/// props.create("my float", "my floating point value", None, 3.14.into()).unwrap();
/// props.set("my float", 2.71.into()).unwrap();
/// ```
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct Properties {
    props: HashMap<String, Property>,
}
impl Properties {
    pub fn new(name: &str, node_type: &str) -> Self {
        let mut properties = Self::default();
        properties
            .create(
                "name",
                "name of the optical element",
                Some(vec![PropCondition::NonEmptyString]),
                name.into(),
            )
            .unwrap();
        properties
            .create(
                "node_type",
                "specific optical type of this node",
                Some(vec![PropCondition::NonEmptyString]),
                node_type.into(),
            )
            .unwrap();
        properties
            .create("inverted", "inverse propagation?", None, false.into())
            .unwrap();
        properties
    }
    /// Create a new property with the given name.
    ///
    /// # Errors
    ///
    /// This function will return an [`OpossumError`] if a property with the same name was already created before.
    pub fn create(
        &mut self,
        name: &str,
        description: &str,
        conditions: Option<Vec<PropCondition>>,
        value: Proptype,
    ) -> OpmResult<()> {
        let new_property = Property {
            prop: value,
            description: description.into(),
            conditions,
        };
        if self.props.insert(name.into(), new_property).is_some() {
            Err(OpossumError::Properties(format!(
                "property {} already created",
                name
            )))
        } else {
            Ok(())
        }
    }
    /// Set the value of the property with the given name.
    ///
    /// # Errors
    ///
    /// This function will return an [`OpossumError`] if
    ///   - the property with the given name does not exist (i.e. has not been created before).
    ///   - property conditions defined during creation are not met.
    pub fn set(&mut self, name: &str, value: Proptype) -> OpmResult<()> {
        let mut property = self
            .props
            .get(name)
            .ok_or(OpossumError::Properties(format!(
                "property {} does not exist",
                name
            )))?
            .clone();
        property.set_value(value)?;
        self.props.insert(name.into(), property);
        Ok(())
    }
    pub fn set_internal(&mut self, name: &str, value: Proptype) -> OpmResult<()> {
        let mut property = self
            .props
            .get(name)
            .ok_or(OpossumError::Properties(format!(
                "property {} does not exist",
                name
            )))?
            .clone();
        property.set_value_internal(value)?;
        self.props.insert(name.into(), property);
        Ok(())
    }
    /// Returns the iter of this [`Properties`].
    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, String, Property> {
        self.props.iter()
    }
    /// Return `true`if a property with the given name exists.
    pub fn contains(&self, key: &str) -> bool {
        self.props.contains_key(key)
    }
    /// Return the value of the given property.
    ///
    /// # Errors
    ///
    /// This function will return an error if the property with the given name does not exist.
    pub fn get(&self, name: &str) -> OpmResult<&Proptype> {
        if let Some(prop) = self.props.get(name) {
            Ok(prop.prop())
        } else {
            Err(OpossumError::Properties(format!(
                "property {} does not exist",
                name
            )))
        }
    }
    /// Return the value of a boolean property.
    ///
    /// This is convenience function for easier access.
    ///
    /// # Errors
    ///
    /// This function will return an error if the property with the given name does not exist.
    pub fn get_bool(&self, name: &str) -> OpmResult<Option<bool>> {
        if let Some(property) = self.props.get(name) {
            if let Proptype::Bool(value) = property.prop {
                Ok(Some(value))
            } else {
                Err(OpossumError::Other("not a bool property".into()))
            }
        } else {
            Ok(None)
        }
    }

    /// Returns the name property of this node.
    ///
    /// # Errors
    ///
    /// This function will return an error if the property "name" and the property "node_type" does not exist.
    pub fn name(&self) -> OpmResult<&str> {
        if let Proptype::String(name) = &self.get("name").unwrap() {
            Ok(name)
        } else {
            self.node_type()
        }
    }

    /// Returns the node-type property of this node.
    ///
    /// # Errors
    ///
    /// This function will return an error if the property "node_type" does not exist.
    pub fn node_type(&self) -> OpmResult<&str> {
        if let Proptype::String(node_type) = &self.get("node_type").unwrap() {
            Ok(node_type)
        } else {
            Err(OpossumError::Properties(
                "Property: \"node_type\" not set!".into(),
            ))
        }
    }

    /// Returns the inversion property of thie node.
    ///
    /// # Panics
    ///
    /// Panics if the property does not exist.
    pub fn inverted(&self) -> bool {
        self.get_bool("inverted").unwrap().unwrap()
    }
}

/// (optical) Property
///
/// A property consists of the actual value (stored as [`Proptype`]), a description and optionally a list of value conditions
/// (such as "GreaterThan", "NonEmptyString", etc.)
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(transparent)]
pub struct Property {
    prop: Proptype,
    #[serde(skip)]
    description: String,
    #[serde(skip)]
    conditions: Option<Vec<PropCondition>>,
}
impl Property {
    /// Returns a reference to the actual property value (expressed as [`Proptype`] prop of this [`Property`].
    pub fn prop(&self) -> &Proptype {
        &self.prop
    }
    /// Returns a reference to the description of this [`Property`].
    pub fn description(&self) -> &str {
        self.description.as_ref()
    }
    /// Sets the value of this [`Property`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the property conditions are  not met.
    pub fn set_value(&mut self, prop: Proptype) -> OpmResult<()> {
        if let Some(conditions) = &self.conditions {
            if conditions.contains(&PropCondition::InternalOnly) {
                return Err(OpossumError::Properties(
                    "property is internally used and public read-only".into(),
                ));
            }
        }
        if mem::discriminant(&self.prop) != mem::discriminant(&prop) {
            return Err(OpossumError::Properties("incompatible value types".into()));
        }
        self.check_conditions(&prop)?;
        self.prop = prop;
        Ok(())
    }
    pub fn set_value_internal(&mut self, prop: Proptype) -> OpmResult<()> {
        self.check_conditions(&prop)?;
        self.prop = prop;
        Ok(())
    }
    fn check_conditions(&self, prop: &Proptype) -> OpmResult<()> {
        if let Some(conditions) = &self.conditions {
            for condition in conditions.iter() {
                match condition {
                    PropCondition::NonEmptyString => {
                        if let Proptype::String(s) = prop.clone() {
                            if s.is_empty() {
                                return Err(OpossumError::Properties(
                                    "string value must not be empty".into(),
                                ));
                            }
                        }
                    }
                    PropCondition::GreaterThan(limit) => match prop {
                        Proptype::I32(val) => {
                            if val.as_f64() <= *limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be > {}",
                                    limit
                                )));
                            }
                        }
                        Proptype::F64(val) => {
                            if val <= limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be > {}",
                                    limit
                                )));
                            }
                        }
                        _ => {}
                    },
                    PropCondition::LessThan(limit) => match prop {
                        Proptype::I32(val) => {
                            if val.as_f64() >= *limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be < {}",
                                    limit
                                )));
                            }
                        }
                        Proptype::F64(val) => {
                            if val >= limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be < {}",
                                    limit
                                )));
                            }
                        }
                        _ => {}
                    },
                    PropCondition::GreaterThanEqual(limit) => match prop {
                        Proptype::I32(val) => {
                            if val.as_f64() < *limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be >= {}",
                                    limit
                                )));
                            }
                        }
                        Proptype::F64(val) => {
                            if val < limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be >= {}",
                                    limit
                                )));
                            }
                        }
                        _ => {}
                    },
                    PropCondition::LessThanEqual(limit) => match prop {
                        Proptype::I32(val) => {
                            if val.as_f64() > *limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be <= {}",
                                    limit
                                )));
                            }
                        }
                        Proptype::F64(val) => {
                            if val > limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be <= {}",
                                    limit
                                )));
                            }
                        }
                        _ => {}
                    },
                    PropCondition::InternalOnly => {}
                }
            }
        }
        Ok(())
    }
}
impl From<bool> for Proptype {
    fn from(value: bool) -> Self {
        Proptype::Bool(value)
    }
}
impl From<f64> for Proptype {
    fn from(value: f64) -> Self {
        Proptype::F64(value)
    }
}
impl From<String> for Proptype {
    fn from(value: String) -> Self {
        Proptype::String(value)
    }
}
impl From<&str> for Proptype {
    fn from(value: &str) -> Self {
        Proptype::String(value.to_string())
    }
}
impl From<i32> for Proptype {
    fn from(value: i32) -> Self {
        Proptype::I32(value)
    }
}
impl From<Uuid> for Proptype {
    fn from(value: Uuid) -> Self {
        Proptype::Uuid(value)
    }
}
#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Proptype {
    String(String),
    I32(i32),
    F64(f64),
    Bool(bool),
    LightData(Option<LightData>),
    OpticGraph(OpticGraph),
    FilterType(FilterType),
    SpectrometerType(SpectrometerType),
    Metertype(Metertype),
    GroupPortMap(PortMap),
    Uuid(Uuid),
}
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum PropCondition {
    NonEmptyString,
    InternalOnly, // DO NOT USE YET (deserialization problems)
    GreaterThan(f64),
    LessThan(f64),
    GreaterThanEqual(f64),
    LessThanEqual(f64),
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn property_set_different_type() {
        let mut prop = Property {
            prop: Proptype::Bool(true),
            description: "".into(),
            conditions: None,
        };
        assert!(prop.set_value(Proptype::Bool(false)).is_ok());
        assert!(prop.set_value(Proptype::F64(3.14)).is_err());
    }
}
