use std::collections::HashMap;

// use serde::Serialize;
use serde_derive::{Deserialize, Serialize};

use crate::{error::OpossumError, lightdata::LightData, optical::OpticGraph};

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
    pub fn get_bool(&self, name: &str) -> Result<Option<bool>, OpossumError> {
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
// impl Serialize for Properties {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer {
//         serializer.serialize_newtype_struct("properties", &self.props)
//     }
// }
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(transparent)]
pub struct Property {
    pub prop: Proptype,
}

// impl Serialize for Property {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer {
//         serializer.serialize_newtype_struct("property", &self.prop)
//     }
// }
#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Proptype {
    String(String),
    I32(i32),
    F64(f64),
    Bool(bool),
    LightData(Option<LightData>),
    #[serde(skip)]
    OpticGraph(OpticGraph),
}
