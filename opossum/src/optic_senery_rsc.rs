#![warn(missing_docs)]
use crate::refractive_index::{refr_index_vaccuum, RefractiveIndexType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Structure handling scenery wide resources (e.g. ambient medium)
pub struct SceneryResources {
    /// Refractive index of the ambient medium
    pub ambient_refr_index: RefractiveIndexType,
}

impl Default for SceneryResources {
    fn default() -> Self {
        Self {
            ambient_refr_index: refr_index_vaccuum(),
        }
    }
}
