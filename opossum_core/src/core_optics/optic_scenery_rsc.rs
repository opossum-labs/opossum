#![warn(missing_docs)]
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::material::Material;

/// Structure handling scenery wide resources (e.g. ambient medium)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SceneryResources {
    /// Refractive index of the ambient medium
    #[schema(value_type=())]
    pub ambient_material: Material,
}
impl Default for SceneryResources {
    fn default() -> Self {
        Self {
            ambient_material: crate::material_vaccuum(),
        }
    }
}
