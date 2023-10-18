use plotters::prelude::LogScalable;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    nodes::{FilterType, Metertype, PortMap, SpectrometerType},
    optic_graph::OpticGraph,
};
/// A general set of (optical) properties.
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct Properties {
    props: HashMap<String, Property>,
}
impl Properties {
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
    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, String, Property> {
        self.props.iter()
    }
    pub fn contains(&self, key: &str) -> bool {
        self.props.contains_key(key)
    }
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
}

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
    pub fn prop(&self) -> &Proptype {
        &self.prop
    }
    pub fn description(&self) -> &str {
        self.description.as_ref()
    }
    pub fn set_value(&mut self, prop: Proptype) -> OpmResult<()> {
        if let Some(conditions) = &self.conditions {
            if conditions.contains(&PropCondition::InternalOnly) {
                return Err(OpossumError::Properties(
                    "property is internally used and public read-only".into(),
                ));
            }
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
