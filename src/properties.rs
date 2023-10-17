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
    pub fn create(&mut self, name: &str, description: &str, value: Proptype) -> OpmResult<()> {
        let new_property = Property {
            prop: value,
            description: description.into(),
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
        property.set_value(value);
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
}
impl Property {
    pub fn prop(&self) -> &Proptype {
        &self.prop
    }
    pub fn description(&self) -> &str {
        self.description.as_ref()
    }

    pub fn set_value(&mut self, prop: Proptype) {
        self.prop = prop;
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
impl From<OpticGraph> for Proptype {
    fn from(value: OpticGraph) -> Self {
        Proptype::OpticGraph(value)
    }
}
impl From<FilterType> for Proptype {
    fn from(value: FilterType) -> Self {
        Proptype::FilterType(value)
    }
}
impl From<SpectrometerType> for Proptype {
    fn from(value: SpectrometerType) -> Self {
        Proptype::SpectrometerType(value)
    }
}
impl From<Metertype> for Proptype {
    fn from(value: Metertype) -> Self {
        Proptype::Metertype(value)
    }
}
impl From<PortMap> for Proptype {
    fn from(value: PortMap) -> Self {
        Proptype::GroupPortMap(value)
    }
}
impl From<Option<LightData>> for Proptype {
    fn from(value: Option<LightData>) -> Self {
        Proptype::LightData(value)
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
