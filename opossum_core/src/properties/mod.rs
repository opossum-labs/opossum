//! Module for handling node properties
pub mod property;
pub mod proptype;
pub mod validator;

use log::warn;
pub use property::Property;
pub use proptype::Proptype;

use crate::error::{OpmResult, OpossumError};
use crate::material::{LEGACY_REFRACTIVE_INDEX, MATERIAL, Material};
use crate::properties::validator::Validator;
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt::Debug;

use crate::reporting::html_report::HtmlProperty;

/// Carry properties of older `.opm` files over to the name they have today.
///
/// A node created from an `.opm` file is built as a default node first and then updated with the
/// deserialized properties. Since [`Properties::update`] silently ignores keys the default node
/// does not know, a renamed property would leave the node on its default value — a data loss
/// without any error message. This function closes that gap and is the one place where such
/// renames are recorded.
///
/// # Arguments
///
/// * `props` - the freshly deserialized properties, modified in place.
fn migrate_legacy_properties(props: &mut IndexMap<String, Property>) {
    // `refractive index` (a bare index model) became `material` (a whole `Material` carrying it).
    if let Some(legacy) = props.shift_remove(LEGACY_REFRACTIVE_INDEX)
        && !props.contains_key(MATERIAL)
        && let Proptype::RefractiveIndex(index) = legacy.prop()
        && let Ok(material) =
            Property::new(Material::from(index.clone()).into(), String::new(), None)
    {
        props.insert(MATERIAL.to_string(), material);
    }
}

/// A general set of (optical) properties.
///
/// The property system is used for storing node specific parameters (such as focal length, splitting ratio, filter curve, etc ...).
/// Properties have to be created once before they can be set and used.
///
/// Properties keep the order in which they were created, because that is the order a node author
/// chose and the order every listing shows them in (property editor, report, `.opm` file). Sorting
/// them by name instead would tear apart what belongs together — a lens would list its front and
/// rear curvature with unrelated properties in between.
///
/// ## Example
/// ```rust
/// # use opossum_core::properties::Properties;
/// # use opossum_core::error::OpmResult;
/// # fn main() -> OpmResult<()> {
/// let mut props = Properties::default();
///
/// // We create a new property and set its value
/// props.create("my float", "my floating point value", 3.14.into())?;
/// props.set("my float", 2.71.into())?;
///
/// # Ok(())
/// # }
/// ```
#[derive(Default, Serialize, Debug, Clone, PartialEq)]
#[serde(transparent)]
pub struct Properties {
    props: IndexMap<String, Property>,
}
impl<'de> Deserialize<'de> for Properties {
    /// Deserialize [`Properties`] and migrate properties stored under an older name.
    ///
    /// This is the deserialization counterpart of the transparent `Serialize` derive: it reads the
    /// same plain map, but runs [`migrate_legacy_properties`] on it before handing it out.
    ///
    /// # Errors
    ///
    /// This function returns an error if the underlying map of properties cannot be read.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut props = IndexMap::<String, Property>::deserialize(deserializer)?;
        migrate_legacy_properties(&mut props);
        Ok(Self { props })
    }
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
        let new_property = Property::new(value, description.into(), None)?;
        self.props.insert(name.into(), new_property);
        Ok(())
    }
    /// Create a new property with the given name and a given value validator
    ///
    /// This function is similar to the `create` function but allows to set a validator. The given value
    /// is already checked against the validator before the actual creation.
    ///
    /// # Errors
    ///
    /// This function will return an [`OpossumError`] if
    /// - a property with the same name was already created before.
    /// - if the validation of the initial given value fails
    pub fn create_with_validator(
        &mut self,
        name: &str,
        description: &str,
        // validator: Box<dyn Validator>,
        validator: Validator,
        value: Proptype,
    ) -> OpmResult<()> {
        if self.props.contains_key(name) {
            return Err(OpossumError::Properties(format!(
                "property {name} already created",
            )));
        }
        let new_property = Property::new(value, description.into(), Some(validator))?;
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
        property.set_value(value).map_err(|e| {
            OpossumError::Properties(format!("Error setting property `{name}`: {e}"))
        })?;
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
    /// Returns the iter of this [`Properties`], mapped to return an `id_string` for the reports and the actual property in a tuple.
    pub fn props_with_report_id_iter(
        &self,
        node_report_id_str: &str,
    ) -> impl Iterator<Item = (String, &Property)> {
        self.props
            .iter()
            .map(move |(s, p)| (format!("{node_report_id_str}_{s}"), p))
    }
    /// Returns the iter of this [`Properties`], in the order the properties were created.
    #[must_use]
    pub fn iter(&self) -> indexmap::map::Iter<'_, String, Property> {
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
    pub fn html_props(&self, id: &str, report_number: usize) -> Vec<HtmlProperty> {
        let mut html_props: Vec<HtmlProperty> = Vec::new();
        for prop in &self.props {
            if let Ok(html_prop_value) = prop.1.prop().to_html(id, prop.0, report_number) {
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

/// Fault tolerant deserializer.
///
/// If a property cannot be read mark it as "Invalid".
impl<'de> Deserialize<'de> for Properties {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum PropertyEntry {
            Valid(Box<Property>),
            Invalid(serde::de::IgnoredAny),
        }

        let raw_props = BTreeMap::<String, PropertyEntry>::deserialize(deserializer)?;
        let mut props = BTreeMap::new();
        for (key, entry) in raw_props {
            match entry {
                PropertyEntry::Valid(prop) => {
                    props.insert(key, *prop);
                }
                PropertyEntry::Invalid(_) => {
                    warn!("Skipping property '{key}' that failed to parse; keeping default value.");
                }
            }
        }
        Ok(Self { props })
    }
}

impl<'a> IntoIterator for &'a Properties {
    type IntoIter = indexmap::map::Iter<'a, String, Property>;
    type Item = (&'a std::string::String, &'a Property);
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        properties::proptype::AssetRef,
        refractive_index::{RefrIndexConst, RefractiveIndexType},
        utils::test_helper::test_helper::check_logs,
    };
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
    fn properties_get() -> OpmResult<()> {
        let mut props = Properties::default();
        props.create("test", "my description", 1.into())?;
        let prop = props.get("test")?;
        assert_matches!(prop, &Proptype::I32(1));
        assert!(props.get("wrong").is_err());
        Ok(())
    }
    #[test]
    fn properties_get_bool() -> OpmResult<()> {
        let mut props = Properties::default();
        props.create("no bool", "my description", 1.into())?;
        props.create("my bool", "my description", true.into())?;
        props.create("my other bool", "my description", false.into())?;
        assert!(props.get_bool("wrong").is_err());
        assert!(props.get_bool("no bool").is_err());
        assert_eq!(props.get_bool("my bool")?, true);
        assert_eq!(props.get_bool("my other bool")?, false);
        Ok(())
    }
    #[test]
    fn is_empty() -> OpmResult<()> {
        let mut props = Properties::default();
        assert_eq!(props.is_empty(), true);
        props.create("my prop", "my description", 1.into())?;
        assert_eq!(props.is_empty(), false);
        Ok(())
    }
    #[test]
    fn iteration_follows_creation_order() -> OpmResult<()> {
        // Every listing of a node's properties (editor, report, `.opm` file) iterates here, so the
        // order a node author declares its properties in is the order the user sees. Sorting by
        // name would put e.g. a lens' `material` between its `front curvature` and `rear curvature`.
        let mut props = Properties::default();
        props.create("front curvature", "", 1.0.into())?;
        props.create("rear curvature", "", 2.0.into())?;
        props.create("material", "", 3.0.into())?;
        assert_eq!(
            props
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            vec!["front curvature", "rear curvature", "material"]
        );
        Ok(())
    }
    /// Read the refractive index model out of a migrated `material` property.
    ///
    /// # Panics
    ///
    /// Panics if the property is missing or does not hold an embedded [`Material`].
    fn migrated_index_model(props: &Properties) -> OpmResult<RefractiveIndexType> {
        let Proptype::Material(AssetRef::Inline(material)) = props.get(MATERIAL)? else {
            panic!("expected an embedded material property")
        };
        Ok(material.optical.refractive_index.clone())
    }
    #[test]
    fn deserialize_migrates_legacy_refractive_index() -> OpmResult<()> {
        let props: Properties = ron::from_str(
            r#"{"refractive index": RefractiveIndex(Const((refractive_index: 2.0)))}"#,
        )
        .map_err(|e| OpossumError::Other(e.to_string()))?;
        assert!(!props.contains(LEGACY_REFRACTIVE_INDEX));
        assert_eq!(
            migrated_index_model(&props)?,
            RefractiveIndexType::Const(RefrIndexConst::new(2.0)?)
        );
        Ok(())
    }
    #[test]
    fn deserialize_keeps_an_existing_material() -> OpmResult<()> {
        // Should both names ever show up side by side, the already migrated value wins. The
        // material entry is generated rather than spelled out, so it cannot drift apart from the
        // serialized shape of `Material` (which carries a whole asset header).
        let material = ron::to_string(&Proptype::from(Material::from(RefractiveIndexType::Const(
            RefrIndexConst::new(3.0)?,
        ))))
        .map_err(|e| OpossumError::Other(e.to_string()))?;
        let props: Properties = ron::from_str(&format!(
            r#"{{
                "refractive index": RefractiveIndex(Const((refractive_index: 2.0))),
                "material": {material},
            }}"#
        ))
        .map_err(|e| OpossumError::Other(e.to_string()))?;
        assert!(!props.contains(LEGACY_REFRACTIVE_INDEX));
        assert_eq!(
            migrated_index_model(&props)?,
            RefractiveIndexType::Const(RefrIndexConst::new(3.0)?)
        );
        Ok(())
    }
    #[test]
    fn deserialize_leaves_other_properties_untouched() -> OpmResult<()> {
        let props: Properties = ron::from_str(r#"{"my float": F64(3.14)}"#)
            .map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(props.nr_of_props(), 1);
        assert_matches!(props.get("my float")?, &Proptype::F64(_));
        Ok(())
    }
    #[test]
    fn html_props() -> OpmResult<()> {
        let mut props = Properties::default();
        props.create("my prop", "my description", 1.into())?;
        testing_logger::setup();
        let html_props = props.html_props("test123", 0);
        let html_props = html_props
            .first()
            .ok_or_else(|| OpossumError::Other("no properties found".to_string()))?;
        check_logs(Level::Warn, vec![]);
        assert_eq!(html_props.name, "my prop");
        assert_eq!(html_props.description, "my description");
        assert_eq!(html_props.prop_value, "1");
        let html_props = props.html_props("test123", 0);
        assert_eq!(html_props.len(), 1);
        Ok(())
    }
}
