//! Module for handling node properties.
pub mod property;
pub mod proptype;
pub mod validator;

use log::warn;
pub use property::Property;
pub use proptype::Proptype;

use crate::{
    error::{OpmResult, OpossumError},
    properties::validator::Validator,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// A general set of optical properties.
///
/// Properties keep the order in which they were created.
#[derive(Default, Serialize, Debug, Clone, PartialEq)]
#[serde(transparent)]
pub struct Properties {
    props: IndexMap<String, Property>,
}

impl Properties {
    /// Helper to insert a property ensuring unique keys.
    fn insert_property(
        &mut self,
        name: &str,
        description: &str,
        value: Proptype,
        validator: Option<Validator>,
        read_only: bool,
    ) -> OpmResult<()> {
        if self.props.contains_key(name) {
            return Err(OpossumError::Properties(format!(
                "Property '{name}' already created"
            )));
        }
        let property = Property::new_with_access(value, description.into(), validator, read_only)?;
        self.props.insert(name.into(), property);
        Ok(())
    }

    /// Create a new mutable property with the given name.
    ///
    /// # Errors
    /// Returns an [`OpossumError`] if a property with the same name already exists.
    pub fn create(&mut self, name: &str, description: &str, value: Proptype) -> OpmResult<()> {
        self.insert_property(name, description, value, None, false)
    }

    /// Create a new mutable property equipped with a validator.
    ///
    /// # Errors
    /// Returns an [`OpossumError`] if the property already exists or validation of the initial value fails.
    pub fn create_with_validator(
        &mut self,
        name: &str,
        description: &str,
        validator: Validator,
        value: Proptype,
    ) -> OpmResult<()> {
        self.insert_property(name, description, value, Some(validator), false)
    }

    /// Create a new read-only property with the given name.
    ///
    /// # Errors
    /// Returns an [`OpossumError`] if a property with the same name already exists.
    pub fn create_read_only(
        &mut self,
        name: &str,
        description: &str,
        value: Proptype,
    ) -> OpmResult<()> {
        self.insert_property(name, description, value, None, true)
    }

    /// Create a new read-only property equipped with a validator.
    ///
    /// # Errors
    /// Returns an [`OpossumError`] if the property already exists or initial validation fails.
    pub fn create_read_only_with_validator(
        &mut self,
        name: &str,
        description: &str,
        validator: Validator,
        value: Proptype,
    ) -> OpmResult<()> {
        self.insert_property(name, description, value, Some(validator), true)
    }

    /// Returns the number of registered properties.
    #[must_use]
    pub fn nr_of_props(&self) -> usize {
        self.props.len()
    }

    /// Checks whether a property exists and is marked as read-only.
    ///
    /// # Errors
    /// Returns an error if the property does not exist.
    pub fn is_read_only(&self, name: &str) -> OpmResult<bool> {
        self.props
            .get(name)
            .map(Property::is_read_only)
            .ok_or_else(|| OpossumError::Properties(format!("Property '{name}' does not exist")))
    }

    /// Checks whether a property at the specified index is marked as read-only.
    ///
    /// # Errors
    /// Returns an error if the index is out of bounds.
    pub fn is_read_only_by_index(&self, index: usize) -> OpmResult<bool> {
        self.props
            .get_index(index)
            .map(|(_, p)| p.is_read_only())
            .ok_or_else(|| {
                OpossumError::Properties(format!("Property index {index} is out of bounds"))
            })
    }

    /// Set the value of an existing property by name.
    ///
    /// # Errors
    /// Returns an error if the property does not exist, is read-only, or fails validation.
    pub fn set(&mut self, name: &str, value: Proptype) -> OpmResult<()> {
        let property = self
            .props
            .get_mut(name)
            .ok_or_else(|| OpossumError::Properties(format!("Property '{name}' does not exist")))?;
        property.set_value(value).map_err(|e| {
            OpossumError::Properties(format!("Error setting property '{name}': {e}"))
        })?;
        Ok(())
    }

    /// Sets the value of a property bypassing the `read_only` restriction.
    ///
    /// Used by simulation routines to write calculated results into read-only output properties.
    ///
    /// # Errors
    /// Returns an error if the property does not exist, types are incompatible, or validation fails.
    pub fn set_internal(&mut self, name: &str, value: Proptype) -> OpmResult<()> {
        let property = self
            .props
            .get_mut(name)
            .ok_or_else(|| OpossumError::Properties(format!("Property '{name}' does not exist")))?;
        property.set_internal_value(value).map_err(|e| {
            OpossumError::Properties(format!("Error setting internal property '{name}': {e}"))
        })?;
        Ok(())
    }

    /// Update [`Properties`] from another [`Properties`] instance.
    ///
    /// Properties existing in both sets are updated. Read-only target properties and non-existent keys
    /// are skipped silently.
    pub fn update(&mut self, new_properties: &Self) {
        for (name, prop) in &new_properties.props {
            let _ = self.set(name, prop.prop().clone());
        }
    }

    /// Look up the numeric index of a property by name.
    ///
    /// Allows optimization routines to cache parameter indices prior to entering hot execution loops.
    #[must_use]
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.props.get_index_of(name)
    }

    /// Returns a reference to the property value at the given index.
    ///
    /// # Errors
    /// Returns an error if the index is out of bounds.
    pub fn get_by_index(&self, index: usize) -> OpmResult<&Proptype> {
        self.props
            .get_index(index)
            .map(|(_, p)| p.prop())
            .ok_or_else(|| {
                OpossumError::Properties(format!("Property index {index} is out of bounds"))
            })
    }

    /// Returns a reference to the full [`Property`] at the given index.
    ///
    /// # Errors
    /// Returns an error if the index is out of bounds.
    pub fn get_property_by_index(&self, index: usize) -> OpmResult<&Property> {
        self.props.get_index(index).map(|(_, p)| p).ok_or_else(|| {
            OpossumError::Properties(format!("Property index {index} is out of bounds"))
        })
    }

    /// Sets the property value at the specified index.
    ///
    /// # Errors
    /// Returns an error if the index is out of bounds, the property is read-only, or validation fails.
    pub fn set_by_index(&mut self, index: usize, value: Proptype) -> OpmResult<()> {
        let (name, property) = self.props.get_index_mut(index).ok_or_else(|| {
            OpossumError::Properties(format!("Property index {index} is out of bounds"))
        })?;
        property.set_value(value).map_err(|e| {
            OpossumError::Properties(format!(
                "Error setting property at index {index} ('{name}'): {e}"
            ))
        })
    }

    /// Sets the property value at the specified index bypassing the `read_only` flag.
    ///
    /// # Errors
    /// Returns an error if the index is out of bounds, types are mismatched, or validation fails.
    pub fn set_internal_by_index(&mut self, index: usize, value: Proptype) -> OpmResult<()> {
        let (name, property) = self.props.get_index_mut(index).ok_or_else(|| {
            OpossumError::Properties(format!("Property index {index} is out of bounds"))
        })?;
        property.set_internal_value(value).map_err(|e| {
            OpossumError::Properties(format!(
                "Error setting internal property at index {index} ('{name}'): {e}"
            ))
        })
    }

    /// Read a numeric scalar as `f64` by property name.
    ///
    /// # Errors
    /// Returns an error if the property does not exist or is not numeric.
    pub fn get_f64(&self, name: &str) -> OpmResult<f64> {
        let prop = self.get(name)?;
        prop.as_f64().ok_or_else(|| {
            OpossumError::Properties(format!("Property '{name}' is not a numeric scalar"))
        })
    }

    /// Set a numeric scalar as `f64` by property name.
    ///
    /// # Errors
    /// Returns an error if the property does not exist, is read-only, or fails validation.
    pub fn set_f64(&mut self, name: &str, value: f64) -> OpmResult<()> {
        let property = self
            .props
            .get_mut(name)
            .ok_or_else(|| OpossumError::Properties(format!("Property '{name}' does not exist")))?;
        property.set_f64(value).map_err(|e| {
            OpossumError::Properties(format!("Error setting scalar value on '{name}': {e}"))
        })
    }

    /// Set a numeric scalar as `f64` by property name bypassing the `read_only` flag.
    ///
    /// # Errors
    /// Returns an error if the property does not exist, is not numeric, or fails validation.
    pub fn set_internal_f64(&mut self, name: &str, value: f64) -> OpmResult<()> {
        let property = self
            .props
            .get_mut(name)
            .ok_or_else(|| OpossumError::Properties(format!("Property '{name}' does not exist")))?;
        property.set_internal_f64(value).map_err(|e| {
            OpossumError::Properties(format!(
                "Error setting internal scalar value on '{name}': {e}"
            ))
        })
    }

    /// Read a numeric scalar as `f64` directly by index without string lookup.
    ///
    /// # Errors
    /// Returns an error if the index is out of bounds or the property is not numeric.
    pub fn get_f64_by_index(&self, index: usize) -> OpmResult<f64> {
        let (_, property) = self.props.get_index(index).ok_or_else(|| {
            OpossumError::Properties(format!("Property index {index} is out of bounds"))
        })?;
        property.prop().as_f64().ok_or_else(|| {
            OpossumError::Properties(format!("Property at index {index} is not a numeric scalar"))
        })
    }

    /// Set a numeric scalar as `f64` directly by index without string lookup.
    ///
    /// # Errors
    /// Returns an error if the index is out of bounds, the property is read-only, or validation fails.
    pub fn set_f64_by_index(&mut self, index: usize, value: f64) -> OpmResult<()> {
        let (name, property) = self.props.get_index_mut(index).ok_or_else(|| {
            OpossumError::Properties(format!("Property index {index} is out of bounds"))
        })?;
        property.set_f64(value).map_err(|e| {
            OpossumError::Properties(format!(
                "Error setting scalar value at index {index} ('{name}'): {e}"
            ))
        })
    }

    /// Set a numeric scalar as `f64` directly by index bypassing the `read_only` flag.
    ///
    /// # Errors
    /// Returns an error if the index is out of bounds, not numeric, or validation fails.
    pub fn set_internal_f64_by_index(&mut self, index: usize, value: f64) -> OpmResult<()> {
        let (name, property) = self.props.get_index_mut(index).ok_or_else(|| {
            OpossumError::Properties(format!("Property index {index} is out of bounds"))
        })?;
        property.set_internal_f64(value).map_err(|e| {
            OpossumError::Properties(format!(
                "Error setting internal scalar value at index {index} ('{name}'): {e}"
            ))
        })
    }

    /// Returns an iterator yielding `(report_id, &Property)` tuples.
    pub fn props_with_report_id_iter(
        &self,
        node_report_id_str: &str,
    ) -> impl Iterator<Item = (String, &Property)> {
        self.props
            .iter()
            .map(move |(s, p)| (format!("{node_report_id_str}_{s}"), p))
    }

    /// Returns an iterator over the properties in insertion order.
    #[must_use]
    pub fn iter(&self) -> indexmap::map::Iter<'_, String, Property> {
        self.props.iter()
    }

    /// Returns `true` if no properties are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.props.is_empty()
    }

    /// Returns `true` if a property with the given name exists.
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.props.contains_key(key)
    }

    /// Return the value of the given property.
    ///
    /// # Errors
    /// Returns an error if the property with the given name does not exist.
    pub fn get(&self, name: &str) -> OpmResult<&Proptype> {
        self.props.get(name).map_or_else(
            || {
                Err(OpossumError::Properties(format!(
                    "Property '{name}' does not exist"
                )))
            },
            |prop| Ok(prop.prop()),
        )
    }

    /// Return the value of a boolean property.
    ///
    /// # Errors
    /// Returns an error if the property does not exist or is not a boolean variant.
    pub fn get_bool(&self, name: &str) -> OpmResult<bool> {
        self.props.get(name).map_or_else(
            || {
                Err(OpossumError::Properties(format!(
                    "Property '{name}' does not exist"
                )))
            },
            |property| {
                if let Proptype::Bool(value) = property.prop() {
                    Ok(*value)
                } else {
                    Err(OpossumError::Properties(format!(
                        "Property '{name}' is not a boolean property"
                    )))
                }
            },
        )
    }
}

/// Fault-tolerant deserializer that preserves property creation order.
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

        let raw_props = IndexMap::<String, PropertyEntry>::deserialize(deserializer)?;
        let mut props = IndexMap::with_capacity(raw_props.len());
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
    type Item = (&'a String, &'a Property);
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
        assert!(props.get_bool("my bool")?);
        assert!(!props.get_bool("my other bool")?);
        Ok(())
    }
    #[test]
    fn is_empty() -> OpmResult<()> {
        let mut props = Properties::default();
        assert!(props.is_empty());
        props.create("my prop", "my description", 1.into())?;
        assert!(!props.is_empty());
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
    #[test]
    fn deserialize_leaves_other_properties_untouched() -> OpmResult<()> {
        let props: Properties = ron::from_str(r#"{"my float": F64(3.14)}"#)
            .map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(props.nr_of_props(), 1);
        assert_matches!(props.get("my float")?, &Proptype::F64(_));
        Ok(())
    }
    #[test]
    fn properties_create_with_validator() {
        let mut props = Properties::default();
        let res = props.create_with_validator(
            "focal_length",
            "Focal length in meters",
            Validator::NumericIsPositive,
            (-5.0).into(),
        );
        assert!(res.is_err());

        let res_ok = props.create_with_validator(
            "focal_length",
            "Focal length in meters",
            Validator::NumericIsPositive,
            5.0.into(),
        );
        assert!(res_ok.is_ok());
    }

    #[test]
    fn properties_set_validation() -> OpmResult<()> {
        let mut props = Properties::default();
        props.create_with_validator(
            "radius",
            "Curvature radius",
            Validator::NumericIsPositive,
            10.0.into(),
        )?;

        assert!(props.set("radius", 15.0.into()).is_ok());
        assert!(props.set("radius", (-2.0).into()).is_err());
        assert!(props.set("unknown", 1.0.into()).is_err());
        Ok(())
    }

    #[test]
    fn properties_update() -> OpmResult<()> {
        let mut target = Properties::default();
        target.create("prop1", "first", 10.into())?;
        target.create("prop2", "second", 20.into())?;

        let mut source = Properties::default();
        source.create("prop2", "second new", 99.into())?;
        source.create("prop3", "unrelated", 100.into())?;

        target.update(&source);

        assert_eq!(target.get("prop1")?, &Proptype::I32(10));
        assert_eq!(target.get("prop2")?, &Proptype::I32(99));
        assert!(!target.contains("prop3"));
        Ok(())
    }

    #[test]
    fn deserialization_preserves_insertion_order() -> OpmResult<()> {
        let ron_data = r#"
        {
            "z_param": F64(1.0),
            "a_param": F64(2.0),
            "m_param": F64(3.0)
        }
        "#;
        let props: Properties =
            ron::from_str(ron_data).map_err(|e| OpossumError::Other(e.to_string()))?;

        let keys: Vec<&str> = props.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["z_param", "a_param", "m_param"]);
        Ok(())
    }
    #[test]
    fn read_only_properties_workflow() -> OpmResult<()> {
        let mut props = Properties::default();
        props.create("focal_length", "Variable focal length", 50.0.into())?;
        props.create_read_only("spot_radius", "Calculated spot radius", 0.002.into())?;

        assert!(!props.is_read_only("focal_length")?);
        assert!(props.is_read_only("spot_radius")?);

        // Optimizer modifies parameter: OK
        assert!(props.set("focal_length", 52.5.into()).is_ok());

        // Optimizer mistakenly modifies read-only detector output: Err
        assert!(props.set("spot_radius", 0.001.into()).is_err());

        // Simulation engine writes output: OK
        assert!(props.set_internal("spot_radius", 0.0018.into()).is_ok());
        assert_eq!(props.get_f64("spot_radius")?, 0.0018);
        Ok(())
    }

    #[test]
    fn index_based_optimizer_loop() -> OpmResult<()> {
        use uom::si::f64::Length;
        use uom::si::length::meter;

        let mut props = Properties::default();
        props.create(
            "thickness",
            "Center thickness",
            Length::new::<meter>(0.01).into(),
        )?;
        props.create_read_only("rms_wavefront", "RMS wavefront error", 0.0001.into())?;

        // Cache indices prior to hot loop
        let idx_thickness = props.index_of("thickness").expect("Property must exist");
        let idx_rms = props
            .index_of("rms_wavefront")
            .expect("Property must exist");

        assert_eq!(props.is_read_only_by_index(idx_thickness)?, false);
        assert_eq!(props.is_read_only_by_index(idx_rms)?, true);

        // Simulated optimizer iterations without string lookups
        for step in 1..=5 {
            let trial_thickness = 0.010 + (step as f64) * 0.001;
            props.set_f64_by_index(idx_thickness, trial_thickness)?;

            // Internal simulation engine calculates response
            let simulated_rms = 0.0001 / (step as f64);
            props.set_internal_f64_by_index(idx_rms, simulated_rms)?;

            // Merit function evaluates response
            let merit_val = props.get_f64_by_index(idx_rms)?;
            assert!((merit_val - simulated_rms).abs() < 1e-12);
        }

        assert!((props.get_f64_by_index(idx_thickness)? - 0.015).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn deserialization_maintains_insertion_order() -> OpmResult<()> {
        let ron_data = r#"
        {
            "front_curvature": F64(100.0),
            "thickness": F64(5.0),
            "rear_curvature": F64(-100.0)
        }
        "#;
        let props: Properties =
            ron::from_str(ron_data).map_err(|e| OpossumError::Other(e.to_string()))?;

        let keys: Vec<&str> = props.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["front_curvature", "thickness", "rear_curvature"]);
        Ok(())
    }
}
