//! [`LightResult`] represents the [`LightData`], which arrives at a given (`OpticPort`)[`OpticPorts`] of an optical node.

use crate::lightdata::LightData;
use std::collections::HashMap;

pub type LightDings<T> = HashMap<String, T>;

pub type LightResult = LightDings<LightData>;
