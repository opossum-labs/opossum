use serde::{Deserialize, Serialize};

use crate::{
    lightdata::LightData, nodes::FilterType, properties::Proptype, ray::SplittingConfig,
    refractive_index::RefractiveIndexType,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EnumProxy<T> {
    pub value: T,
}

impl From<EnumProxy<SplittingConfig>> for Proptype {
    fn from(value: EnumProxy<SplittingConfig>) -> Self {
        Self::SplitterType(value)
    }
}

impl From<EnumProxy<FilterType>> for Proptype {
    fn from(value: EnumProxy<FilterType>) -> Self {
        Self::FilterType(value)
    }
}

impl From<EnumProxy<RefractiveIndexType>> for Proptype {
    fn from(value: EnumProxy<RefractiveIndexType>) -> Self {
        Self::RefractiveIndex(value)
    }
}

impl From<EnumProxy<Option<LightData>>> for Proptype {
    fn from(value: EnumProxy<Option<LightData>>) -> Self {
        Self::LightData(value)
    }
}
