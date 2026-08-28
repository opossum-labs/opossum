use opossum_core::absorption::absorption_model::AbsorptionModel;
use strum::EnumIter;

use crate::components::node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings};

/// Parameter descriptors for the optical extinction coefficient (k) model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum ExtinctionParam {
    /// Dimensionless extinction coefficient k (imaginary part of complex refractive index).
    K,
}

impl From<ExtinctionParam> for InputParam {
    fn from(param: ExtinctionParam) -> Self {
        match param {
            ExtinctionParam::K => InputParam::F64("Extinction coefficient (k)".to_string()),
        }
    }
}

impl IntoInputDataStrings<f64> for ExtinctionParam {
    fn create_value_string(&self, obj: &f64) -> String {
        match self {
            Self::K => obj.to_string(),
        }
    }

    fn create_id_string(&self) -> String {
        match self {
            Self::K => "extinction_k_".to_string(),
        }
    }
}

impl IntoInputData<f64, f64, AbsorptionModel> for ExtinctionParam {
    fn setter_from_obj(&self) -> impl FnMut(&mut f64, f64) {
        move |obj: &mut f64, new_val: f64| {
            // Extinction coefficient must be finite and non-negative
            if new_val >= 0.0 && new_val.is_finite() {
                *obj = new_val;
            }
        }
    }
}
