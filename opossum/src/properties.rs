#![warn(missing_docs)]
//! Module for handling node properties
use crate::{
    aperture::Aperture,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    nodes::{
        FilterType, Metertype, PortMap, Spectrometer, SpectrometerType, SplittingConfig,
        SpotDiagram, WaveFront,
    },
    optic_graph::OpticGraph,
    optic_ports::OpticPorts,
    reporter::{NodeReport, PdfReportable},
};
use genpdf::{elements::TableLayout, style};
use plotters::prelude::LogScalable;
use serde_derive::{Deserialize, Serialize};
use std::fmt::Debug;
use std::{collections::BTreeMap, mem};
use uom::num::Float;
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::meter,
    Dimension, Quantity, Unit, Units,
};
use uuid::Uuid;
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
    /// Creates new [`Properties`].
    ///
    /// This automatically creates some "standard" properties common to all optic nodes (name, node type, inverted, apertures)
    /// # Panics
    ///
    /// Panics theoretically if above properties could not be created.
    #[must_use]
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
                Some(vec![PropCondition::NonEmptyString, PropCondition::ReadOnly]),
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
        if self.props.contains_key(name) {
            return Err(OpossumError::Properties(format!(
                "property {name} already created",
            )));
        }
        let new_property = Property {
            prop: value,
            description: description.into(),
            conditions,
        };
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
                    "property {name} not found"
                )))
            },
            |property| {
                if let Proptype::Bool(value) = property.prop {
                    Ok(value)
                } else {
                    Err(OpossumError::Other("not a bool property".into()))
                }
            },
        )
    }
    /// Returns the name property of this node.
    ///
    /// # Errors
    ///
    /// This function will return an error if the property `name` and the property `node_type` does not exist.
    pub fn name(&self) -> OpmResult<&str> {
        if let Ok(Proptype::String(name)) = &self.get("name") {
            Ok(name)
        } else {
            self.node_type()
        }
    }
    /// Returns the node-type property of this node.
    ///
    /// # Errors
    ///
    /// This function will return an error if the property `node_type` does not exist.
    pub fn node_type(&self) -> OpmResult<&str> {
        if let Ok(Proptype::String(node_type)) = &self.get("node_type") {
            Ok(node_type)
        } else {
            Err(OpossumError::Properties(
                "Property: \"node_type\" not set!".into(),
            ))
        }
    }
    /// Returns the inversion property of thie node.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying property `inverted` does not exist or has the wrong datatype.
    pub fn inverted(&self) -> OpmResult<bool> {
        self.get_bool("inverted")
    }
}

impl<'a> IntoIterator for &'a Properties {
    type IntoIter = std::collections::btree_map::Iter<'a, String, Property>;
    type Item = (&'a std::string::String, &'a Property);
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl PdfReportable for Properties {
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        let mut layout = genpdf::elements::LinearLayout::vertical();
        let mut table = TableLayout::new(vec![1, 3]);
        for property in &self.props {
            let mut table_row = table.row();
            let property_name = genpdf::elements::Paragraph::default()
                .styled_string(format!("{}: ", property.0), style::Effect::Bold)
                .aligned(genpdf::Alignment::Right);
            table_row.push_element(property_name);
            table_row.push_element(property.1.pdf_report()?);
            table_row.push().unwrap();
        }
        layout.push(table);
        Ok(layout)
    }
}
/// (optical) Property
///
/// A property consists of the actual value (stored as [`Proptype`]), a description and optionally a list of value conditions
/// (such as `GreaterThan`, `NonEmptyString`, etc.)
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
    #[must_use]
    pub const fn prop(&self) -> &Proptype {
        &self.prop
    }
    /// Returns a reference to the description of this [`Property`].
    #[must_use]
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
    /// Sets the value unchecked of this [`Property`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the [`Proptype`]s [`PropCondition`]s are not met.
    pub fn set_value_unchecked(&mut self, prop: Proptype) -> OpmResult<()> {
        self.check_conditions(&prop)?;
        self.prop = prop;
        Ok(())
    }
    fn check_conditions(&self, prop: &Proptype) -> OpmResult<()> {
        if let Some(conditions) = &self.conditions {
            for condition in conditions {
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
                                    "value must be > {limit}"
                                )));
                            }
                        }
                        Proptype::F64(val) => {
                            if val <= limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be > {limit}"
                                )));
                            }
                        }
                        _ => {}
                    },
                    PropCondition::LessThan(limit) => match prop {
                        Proptype::I32(val) => {
                            if val.as_f64() >= *limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be < {limit}"
                                )));
                            }
                        }
                        Proptype::F64(val) => {
                            if val >= limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be < {limit}"
                                )));
                            }
                        }
                        _ => {}
                    },
                    PropCondition::GreaterThanEqual(limit) => match prop {
                        Proptype::I32(val) => {
                            if val.as_f64() < *limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be >= {limit}"
                                )));
                            }
                        }
                        Proptype::F64(val) => {
                            if val < limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be >= {limit}"
                                )));
                            }
                        }
                        _ => {}
                    },
                    PropCondition::LessThanEqual(limit) => match prop {
                        Proptype::I32(val) => {
                            if val.as_f64() > *limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be <= {limit}"
                                )));
                            }
                        }
                        Proptype::F64(val) => {
                            if val > limit {
                                return Err(OpossumError::Properties(format!(
                                    "value must be <= {limit}"
                                )));
                            }
                        }
                        _ => {}
                    },
                    PropCondition::InternalOnly => {}
                    PropCondition::ReadOnly => {
                        return Err(OpossumError::Properties("property is read-only".into()));
                    }
                }
            }
        }
        Ok(())
    }
}
impl PdfReportable for Property {
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        self.prop.pdf_report()
    }
}
impl From<bool> for Proptype {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}
impl From<f64> for Proptype {
    fn from(value: f64) -> Self {
        Self::F64(value)
    }
}
impl From<String> for Proptype {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}
impl From<&str> for Proptype {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}
impl From<i32> for Proptype {
    fn from(value: i32) -> Self {
        Self::I32(value)
    }
}
impl From<Uuid> for Proptype {
    fn from(value: Uuid) -> Self {
        Self::Uuid(value)
    }
}
impl From<Length> for Proptype {
    fn from(value: Length) -> Self {
        Self::Length(value)
    }
}
impl From<Energy> for Proptype {
    fn from(value: Energy) -> Self {
        Self::Energy(value)
    }
}
#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Clone)]
/// The type of the [`Property`].
pub enum Proptype {
    /// A string property
    ///
    /// This property makes use of [`PropCondition::NonEmptyString`] for restricting strings to be non-empty.
    String(String),
    /// An integer property
    ///
    /// This property respects the [`PropCondition::LessThan`], [`PropCondition::LessThanEqual`], [`PropCondition::GreaterThan`], and [`PropCondition::GreaterThanEqual`]
    I32(i32),
    /// A float property
    ///
    /// This property respects the [`PropCondition::LessThan`], [`PropCondition::LessThanEqual`], [`PropCondition::GreaterThan`], and [`PropCondition::GreaterThanEqual`]
    F64(f64),
    /// A boolean property
    Bool(bool),
    /// An optional [`LightData`] property
    LightData(Option<LightData>),
    /// A property for storing a complete `OpticGraph` to be used by [`OpticScenery`](crate::OpticScenery).
    OpticGraph(OpticGraph),
    /// Property for storing a [`FilterType`] of an [`IdealFilter`](crate::nodes::IdealFilter) node.
    FilterType(FilterType),
    /// Property for storing a [`SplitterType`] of an [`BeamSplitter`](crate::nodes::BeamSplitter) node.
    SplitterType(SplittingConfig),
    /// Property for storing a [`SpectrometerType`] of a [`Sepctrometer`](crate::nodes::Spectrometer) node.
    SpectrometerType(SpectrometerType),
    /// Property for storing a [`Metertype`] of an [`Energymeter`](crate::nodes::EnergyMeter) node.
    Metertype(Metertype),
    /// Property for storing the external port mapping ([`PortMap`]) of a [`Group`](crate::nodes::NodeGroup) node.
    GroupPortMap(PortMap),
    /// An [`Uuid`] for identifying an optical node.
    Uuid(Uuid),
    /// A property for storing [`OpticPorts`].
    OpticPorts(OpticPorts),
    /// A property for storing an optical [`Aperture`]
    Aperture(Aperture),
    /// This property stores a [`Spectrum`](crate::spectrum::Spectrum)
    Spectrometer(Spectrometer),
    /// This property stores optical [`Rays`](crate::rays::Rays)
    SpotDiagram(SpotDiagram),
    /// This property stores optical [`Rays`](crate::rays::Rays)
    WaveFront(WaveFront),
    /// A (nested set) of Properties
    NodeReport(NodeReport),
    /// a geometrical length
    Length(Length),
    /// an energy value
    Energy(Energy),
}
fn format_value_with_prefix(value: f64) -> String {
    if value.is_nan() {
        return String::from("     nan ");
    }
    if value == f64::INFINITY {
        return String::from("     inf ");
    }
    if value == f64::NEG_INFINITY {
        return String::from("    -inf ");
    }
    if value.abs() < f64::EPSILON {
        return String::from("   0.000 ");
    }
    #[allow(clippy::cast_possible_truncation)]
    let mut exponent = (f64::log10(value.abs()).floor()) as i32;
    if exponent.is_negative() {
        exponent -= 2;
    }
    let exponent = (exponent / 3) * 3;
    let prefix = match exponent {
        -21 => "z",
        -18 => "a",
        -15 => "f",
        -12 => "p",
        -9 => "n",
        -6 => "u",
        -3 => "m",
        0 => "",
        3 => "k",
        6 => "M",
        9 => "G",
        12 => "T",
        15 => "P",
        18 => "E",
        21 => "Z",
        _ => "?",
    };
    format!("{:8.3} {prefix}", value / f64::powi(10.0, exponent))
}
fn format_quantity<D, U, V, N>(_: N, q: Quantity<D, U, V>) -> String
where
    D: Dimension + ?Sized,
    U: Units<V> + ?Sized,
    V: Float + uom::Conversion<V> + Debug,
    N: Unit,
{
    let base_unit = N::abbreviation();
    let base_value = q.value.to_f64().unwrap();
    format!("{}{}", format_value_with_prefix(base_value), base_unit)
}
impl PdfReportable for Proptype {
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        let mut l = genpdf::elements::LinearLayout::vertical();
        match self {
            Self::String(value) => l.push(genpdf::elements::Paragraph::new(value)),
            Self::I32(value) => l.push(genpdf::elements::Paragraph::new(format!("{value}"))),
            Self::F64(value) => l.push(genpdf::elements::Paragraph::new(format!("{value:.6}"))),
            Self::Bool(value) => l.push(genpdf::elements::Paragraph::new(value.to_string())),
            Self::FilterType(value) => l.push(value.pdf_report()?),
            Self::SpectrometerType(value) => l.push(value.pdf_report()?),
            Self::Metertype(value) => l.push(value.pdf_report()?),
            Self::Spectrometer(value) => l.push(value.pdf_report()?),
            Self::SpotDiagram(value) => l.push(value.pdf_report()?),
            Self::WaveFront(value) => l.push(value.pdf_report()?),
            Self::NodeReport(value) => l.push(value.properties().pdf_report()?),
            Self::Length(value) => l.push(genpdf::elements::Paragraph::new(format_quantity(
                meter, *value,
            ))),
            Self::Energy(value) => l.push(genpdf::elements::Paragraph::new(format_quantity(
                joule, *value,
            ))),
            _ => l.push(
                genpdf::elements::Paragraph::default()
                    .styled_string("unknown poperty type", style::Effect::Italic),
            ),
        }
        Ok(l)
    }
}
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
/// An enum defining value constraints for various [`Proptype`]s.
pub enum PropCondition {
    /// Allow only non-empty [`Proptype::String`]s.
    NonEmptyString,
    /// Do not use yet...
    InternalOnly, // DO NOT USE YET (deserialization problems)
    /// This property is readonly. It can only be set during property creation.
    ReadOnly, // can only be set during creation
    /// Restrict integer or float properties to values greater (>) than the given limit.
    GreaterThan(f64),
    /// Restrict integer or float properties to values less (<) than the given limit.
    LessThan(f64),
    /// Restrict integer or float properties to values greater than or equal (>=) the given limit.
    GreaterThanEqual(f64),
    /// Restrict integer or float properties to values less than or equal (<=) the given limit.
    LessThanEqual(f64),
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
    fn properties_node_type() {
        let mut props = Properties::default();
        assert!(props.node_type().is_err());
        props
            .create("node_type", "my description", None, "my node".into())
            .unwrap();
        assert_eq!(props.node_type().unwrap(), "my node");
        let mut props = Properties::default();
        props
            .create("node_type", "my description", None, true.into())
            .unwrap();
        assert!(props.node_type().is_err());
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
    #[test]
    fn property_description() {
        let prop = Property {
            prop: true.into(),
            description: "my description".to_string(),
            conditions: None,
        };
        assert_eq!(prop.description(), "my description");
    }
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
    #[test]
    fn format_value() {
        assert_eq!(format_value_with_prefix(0.0), "   0.000 ");
        assert_eq!(format_value_with_prefix(1.0), "   1.000 ");
        assert_eq!(format_value_with_prefix(999.12345), " 999.123 ");
        assert_eq!(format_value_with_prefix(1001.2345), "   1.001 k");
        assert_eq!(format_value_with_prefix(1234567.12345), "   1.235 M");
        assert_eq!(format_value_with_prefix(1234567890.12345), "   1.235 G");
        assert_eq!(format_value_with_prefix(-1234567890.12345), "  -1.235 G");
        assert_eq!(format_value_with_prefix(0.12345), " 123.450 m");
        assert_eq!(format_value_with_prefix(-0.0000012345), "  -1.235 u");
        assert_eq!(format_value_with_prefix(f64::INFINITY), "     inf ");
        assert_eq!(format_value_with_prefix(f64::NEG_INFINITY), "    -inf ");
        assert_eq!(format_value_with_prefix(f64::NAN), "     nan ");
    }
}
