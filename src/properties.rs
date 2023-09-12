use std::collections::HashMap;

use serde::Serialize;
use serde_derive::Serialize;

#[derive(Default, Serialize, Debug, Clone)]
pub struct Properties {
  props: HashMap<String, Property>
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
    self.props.get(name.into())
  }
}
#[derive(Debug, Clone)]
pub struct Property {
  pub prop: Proptype
}

impl Serialize for Property {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        serializer.serialize_newtype_struct("hallo", &self.prop)
    }
}
#[non_exhaustive]
#[derive(Serialize, Debug, Clone)]
pub enum Proptype {
  String(String),
  I32(i32),
  F64(f64)
}