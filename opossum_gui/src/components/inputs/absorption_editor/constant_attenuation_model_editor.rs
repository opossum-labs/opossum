use dioxus::prelude::*;
use opossum_core::absorption::{
    absorption_constant::AbsConst,
    absorption_model::AbsorptionModel,
};
use strum::EnumIter;

use crate::components::node_editor::inputs::{
    InputParam, IntoInputData, IntoInputDataStrings,
};

/// Parameter descriptors for the constant attenuation model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum ConstantAttenuationParam {
    /// Transmission factor in range [0.0, 1.0].
    TransmissionFactor,
}

impl From<ConstantAttenuationParam> for InputParam {
    fn from(param: ConstantAttenuationParam) -> Self {
        match param {
            ConstantAttenuationParam::TransmissionFactor => {
                InputParam::F64("Transmission factor".to_string())
            }
        }
    }
}

impl IntoInputDataStrings<AbsConst> for ConstantAttenuationParam {
    fn create_value_string(&self, obj: &AbsConst) -> String {
        match self {
            Self::TransmissionFactor => obj.absorption_constant().to_string(),
        }
    }

    fn create_id_string(&self) -> String {
        match self {
            Self::TransmissionFactor => "const_attenuation_factor_".to_string(),
        }
    }
}

impl IntoInputData<f64, AbsConst, AbsorptionModel> for ConstantAttenuationParam {
    fn setter_from_obj(&self) -> impl FnMut(&mut AbsConst, f64) {
        move |obj: &mut AbsConst, new_val: f64| {
            if let Ok(updated) = AbsConst::new(new_val) {
                *obj = updated;
            }
        }
    }
}