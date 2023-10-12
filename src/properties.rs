use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    nodes::{FilterType, Metertype, PortMap, SpectrometerType},
    optic_graph::OpticGraph,
};
/// A general set of (optical) properties.
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct Properties {
    pub props: HashMap<String, Property>,
}

impl Properties {
    pub fn set(&mut self, name: &str, value: Property) -> Option<()> {
        if self.props.insert(name.into(), value).is_some() {
            Some(())
        } else {
            None
        }
    }
    pub fn get(&self, name: &str) -> Option<&Property> {
        self.props.get(name)
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
    pub prop: Proptype,
}

impl From<bool> for Property {
    fn from(value: bool) -> Self {
        Property {
            prop: Proptype::Bool(value),
        }
    }
}

impl From<f64> for Property {
    fn from(value: f64) -> Self {
        Property {
            prop: Proptype::F64(value),
        }
    }
}

impl From<String> for Property {
    fn from(value: String) -> Self {
        Property {
            prop: Proptype::String(value),
        }
    }
}

impl From<&str> for Property {
    fn from(value: &str) -> Self {
        Property {
            prop: Proptype::String(value.to_string()),
        }
    }
}

impl From<i32> for Property {
    fn from(value: i32) -> Self {
        Property {
            prop: Proptype::I32(value),
        }
    }
}
impl From<OpticGraph> for Property {
    fn from(value: OpticGraph) -> Self {
        Property {
            prop: Proptype::OpticGraph(value),
        }
    }
}
impl From<FilterType> for Property {
    fn from(value: FilterType) -> Self {
        Property {
            prop: Proptype::FilterType(value),
        }
    }
}
impl From<SpectrometerType> for Property {
    fn from(value: SpectrometerType) -> Self {
        Property {
            prop: Proptype::SpectrometerType(value),
        }
    }
}
impl From<Metertype> for Property {
    fn from(value: Metertype) -> Self {
        Property {
            prop: Proptype::Metertype(value),
        }
    }
}
impl From<PortMap> for Property {
    fn from(value: PortMap) -> Self {
        Property {
            prop: Proptype::GroupPortMap(value),
        }
    }
}
impl From<Option<LightData>> for Property {
    fn from(value: Option<LightData>) -> Self {
        Property {
            prop: Proptype::LightData(value),
        }
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
}
