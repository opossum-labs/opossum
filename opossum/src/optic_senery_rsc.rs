use crate::refractive_index::{refr_index_vaccuum, RefractiveIndexType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneryResources {
    pub ambient_refr_index: RefractiveIndexType,
}

impl Default for SceneryResources {
    fn default() -> Self {
        Self {
            ambient_refr_index: refr_index_vaccuum(),
        }
    }
}
