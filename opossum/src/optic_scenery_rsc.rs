#![warn(missing_docs)]
use crate::refractive_index::{RefractiveIndexType, refr_index_vaccuum};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Structure handling scenery wide resources (e.g. ambient medium)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SceneryResources {
    /// Refractive index of the ambient medium
    #[schema(value_type=())]
    pub ambient_refr_index: RefractiveIndexType,
}

impl Default for SceneryResources {
    fn default() -> Self {
        Self {
            ambient_refr_index: refr_index_vaccuum(),
        }
    }
}
