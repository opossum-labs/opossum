//! Module for handling node properties
pub mod property;
pub mod proptype;

use log::warn;
pub use property::Property;
pub use proptype::{PropCondition, Proptype};

use crate::error::{OpmResult, OpossumError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Debug;

use self::property::HtmlProperty;

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
#[serde(transparent)]
pub struct Properties {
    props: BTreeMap<String, Property>,
}
impl Properties {
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
        if self.props.contains_key(name) {
            return Err(OpossumError::Properties(format!(
                "property {name} already created",
            )));
        }
        let new_property = Property::new(value, description.into(), conditions);
        self.props.insert(name.into(), new_property);
        Ok(())
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
            .ok_or_else(|| OpossumError::Properties(format!("property {name} does not exist")))?
            .clone();
        property.set_value(value)?;
        self.props.insert(name.into(), property);
        Ok(())
    }
    /// Sets the unchecked value of this [`Properties`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the [`PropCondition`]s of the [`Proptype`] are not met.
    pub fn set_unchecked(&mut self, name: &str, value: Proptype) -> OpmResult<()> {
        let mut property = self
            .props
            .get(name)
            .ok_or_else(|| OpossumError::Properties(format!("property {name} does not exist")))?
            .clone();
        property.set_value_unchecked(value)?;
        self.props.insert(name.into(), property);
        Ok(())
    }
    /// Returns the iter of this [`Properties`].
    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, String, Property> {
        self.props.iter()
    }
    /// Return `true`if a property with the given name exists.
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.props.contains_key(key)
    }
    /// Return the value of the given property.
    ///
    /// # Errors
    ///
    /// This function will return an error if the property with the given name does not exist.
    pub fn get(&self, name: &str) -> OpmResult<&Proptype> {
        self.props.get(name).map_or_else(
            || {
                Err(OpossumError::Properties(format!(
                    "property {name} does not exist"
                )))
            },
            |prop| Ok(prop.prop()),
        )
    }
    /// Return the value of a boolean property.
    ///
    /// This is convenience function for easier access.
    ///
    /// # Errors
    ///
    /// This function will return an error if the property with the given name does not exist.
    pub fn get_bool(&self, name: &str) -> OpmResult<bool> {
        self.props.get(name).map_or_else(
            || {
                Err(OpossumError::Properties(format!(
                    "property {name} does not exist"
                )))
            },
            |property| {
                if let Proptype::Bool(value) = property.prop() {
                    Ok(*value)
                } else {
                    Err(OpossumError::Other("not a bool property".into()))
                }
            },
        )
    }
    /// Returns the inversion property of thie node.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying property `inverted` does not exist or has the wrong datatype.
    pub fn inverted(&self) -> OpmResult<bool> {
        self.get_bool("inverted")
    }
    #[must_use]
    pub fn html_props(&self, node_name: &str) -> Vec<HtmlProperty> {
        let mut html_props: Vec<HtmlProperty> = Vec::new();
        for prop in &self.props {
            if let Ok(html_prop_value) = prop.1.prop().to_html(node_name) {
                let html_prop = HtmlProperty {
                    name: prop.0.to_owned(),
                    description: prop.1.description().into(),
                    prop_value: html_prop_value,
                };
                html_props.push(html_prop);
            } else {
                warn!(
                    "property {} could not be converted to html. Skipping",
                    prop.0.to_owned()
                );
            }
        }
        html_props
    }
}

impl<'a> IntoIterator for &'a Properties {
    type IntoIter = std::collections::btree_map::Iter<'a, String, Property>;
    type Item = (&'a std::string::String, &'a Property);
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use assert_matches::assert_matches;
    #[test]
    fn properties_create() {
        let mut props = Properties::default();
        assert!(props
            .create("test", "my description", None, 1.into())
            .is_ok());
        assert_eq!(props.props.len(), 1);
        assert!(props
            .create("test2", "my description", None, 1.into())
            .is_ok());
        assert_eq!(props.props.len(), 2);
        assert!(props
            .create("test", "my description", None, 2.into())
            .is_err());
        assert_eq!(props.props.len(), 2);
    }
    #[test]
    fn properties_get() {
        let mut props = Properties::default();
        props
            .create("test", "my description", None, 1.into())
            .unwrap();
        let prop = props.get("test").unwrap();
        assert_matches!(prop, &Proptype::I32(1));
        assert!(props.get("wrong").is_err());
    }
    #[test]
    fn properties_get_bool() {
        let mut props = Properties::default();
        props
            .create("no bool", "my description", None, 1.into())
            .unwrap();
        props
            .create("my bool", "my description", None, true.into())
            .unwrap();
        props
            .create("my other bool", "my description", None, false.into())
            .unwrap();
        assert!(props.get_bool("wrong").is_err());
        assert!(props.get_bool("no bool").is_err());
        assert_eq!(props.get_bool("my bool").unwrap(), true);
        assert_eq!(props.get_bool("my other bool").unwrap(), false);
    }
}
