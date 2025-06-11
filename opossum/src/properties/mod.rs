//! Module for handling node properties
pub mod property;
pub mod proptype;

use log::warn;
pub use property::Property;
pub use proptype::Proptype;

use crate::error::{OpmResult, OpossumError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::Path;

use crate::reporting::html_report::HtmlProperty;

/// A general set of (optical) properties.
///
/// The property system is used for storing node specific parameters (such as focal length, splitting ratio, filter curve, etc ...).
/// Properties have to be created once before they can be set and used.
///
/// ## Example
/// ```rust
/// use opossum::properties::Properties;
/// let mut props = Properties::default();
/// props.create("my float", "my floating point value", 3.14.into()).unwrap();
/// props.set("my float", 2.71.into()).unwrap();
/// ```
#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq)]
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
    pub fn create(&mut self, name: &str, description: &str, value: Proptype) -> OpmResult<()> {
        if self.props.contains_key(name) {
            return Err(OpossumError::Properties(format!(
                "property {name} already created",
            )));
        }
        let new_property = Property::new(value, description.into());
        self.props.insert(name.into(), new_property);
        Ok(())
    }
    /// Returns the number of properties that have been set
    #[must_use]
    pub fn nr_of_props(&self) -> usize {
        self.props.len()
    }
    /// Set the value of the property with the given name.
    ///
    /// # Errors
    ///
    /// This function will return an [`OpossumError`] if
    ///   - the property with the given name does not exist (i.e. has not been created before).
    ///   - property conditions defined during creation are not met.
    pub fn set(&mut self, name: &str, value: Proptype) -> OpmResult<()> {
        let property = self
            .props
            .get_mut(name) // Get mutable reference
            .ok_or_else(|| OpossumError::Properties(format!("property {name} does not exist")))?;
        property.set_value(value)?; // set_value would take Proptype by value
        Ok(())
    }
    /// Update [`Properties`] through another [`Properties`] input.
    ///
    /// This functions sets all [`Properties`] from `new_properties` that have already been created in `Self`. Properties not existent
    /// in `Self` are silently ignored.
    pub fn update(&mut self, new_properties: Self) {
        for new_prop in new_properties.props {
            let _ = self.set(&new_prop.0, (*new_prop.1.prop()).clone());
        }
    }
    /// Returns the iter of this [`Properties`].
    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, String, Property> {
        self.props.iter()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.props.is_empty()
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
    #[must_use]
    pub fn html_props(&self, id: &str) -> Vec<HtmlProperty> {
        let mut html_props: Vec<HtmlProperty> = Vec::new();
        for prop in &self.props {
            if let Ok(html_prop_value) = prop.1.prop().to_html(id, prop.0) {
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
    /// Export these [`Properties`] to a of files on disk at the given `report_path`.
    ///
    /// # Errors
    ///
    /// This function will return an error if the underlying implementation for a concrete property
    /// returns an error.
    pub fn export_data(&self, report_path: &Path, id: &str) -> OpmResult<()> {
        for prop in &self.props {
            prop.1
                .export_data(report_path, &format!("{id}_{}", prop.0))?;
        }
        Ok(())
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
    use crate::utils::test_helper::test_helper::check_logs;
    use assert_matches::assert_matches;
    use log::Level;
    #[test]
    fn properties_create() {
        let mut props = Properties::default();
        assert!(props.create("test", "my description", 1.into()).is_ok());
        assert_eq!(props.props.len(), 1);
        assert!(props.create("test2", "my description", 1.into()).is_ok());
        assert_eq!(props.props.len(), 2);
        assert!(props.create("test", "my description", 2.into()).is_err());
        assert_eq!(props.props.len(), 2);
    }
    #[test]
    fn properties_get() {
        let mut props = Properties::default();
        props.create("test", "my description", 1.into()).unwrap();
        let prop = props.get("test").unwrap();
        assert_matches!(prop, &Proptype::I32(1));
        assert!(props.get("wrong").is_err());
    }
    #[test]
    fn properties_get_bool() {
        let mut props = Properties::default();
        props.create("no bool", "my description", 1.into()).unwrap();
        props
            .create("my bool", "my description", true.into())
            .unwrap();
        props
            .create("my other bool", "my description", false.into())
            .unwrap();
        assert!(props.get_bool("wrong").is_err());
        assert!(props.get_bool("no bool").is_err());
        assert_eq!(props.get_bool("my bool").unwrap(), true);
        assert_eq!(props.get_bool("my other bool").unwrap(), false);
    }
    #[test]
    fn is_empty() {
        let mut props = Properties::default();
        assert_eq!(props.is_empty(), true);
        props.create("my prop", "my description", 1.into()).unwrap();
        assert_eq!(props.is_empty(), false);
    }
    #[test]
    fn html_props() {
        let mut props = Properties::default();
        props.create("my prop", "my description", 1.into()).unwrap();
        testing_logger::setup();
        let html_props = props.html_props("test123");
        let html_props = html_props.first().unwrap();
        check_logs(Level::Warn, vec![]);
        assert_eq!(html_props.name, "my prop");
        assert_eq!(html_props.description, "my description");
        assert_eq!(html_props.prop_value, "1");
        let html_props = props.html_props("test123");
        assert_eq!(html_props.len(), 1);
    }
}
