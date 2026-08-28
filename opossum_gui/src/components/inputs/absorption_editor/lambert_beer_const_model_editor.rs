use opossum_core::{
    absorption::{absorption_lb_constant::AbsLBConst, absorption_model::AbsorptionModel},
    num_per_m,
};
use strum::EnumIter;
use uom::si::linear_number_density::per_meter;

use crate::components::node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings};

/// Parameter descriptors for the constant Lambert-Beer absorption model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum LambertBeerConstParam {
    /// Linear absorption coefficient alpha (in m⁻¹).
    Alpha,
}

impl From<LambertBeerConstParam> for InputParam {
    fn from(param: LambertBeerConstParam) -> Self {
        match param {
            LambertBeerConstParam::Alpha => {
                // Configures a numeric field with SI prefix handling for inverse meters (m⁻¹)
                InputParam::SIUnit("Absorption coefficient (α)".to_string(), "m⁻¹".to_string())
            }
        }
    }
}

impl IntoInputDataStrings<AbsLBConst> for LambertBeerConstParam {
    fn create_value_string(&self, obj: &AbsLBConst) -> String {
        match self {
            Self::Alpha => obj.alpha().get::<per_meter>().to_string(),
        }
    }

    fn create_id_string(&self) -> String {
        match self {
            Self::Alpha => "lb_const_alpha_".to_string(),
        }
    }
}

impl IntoInputData<f64, AbsLBConst, AbsorptionModel> for LambertBeerConstParam {
    fn setter_from_obj(&self) -> impl FnMut(&mut AbsLBConst, f64) {
        move |obj: &mut AbsLBConst, new_val: f64| {
            // Validate and update the absorption coefficient using the num_per_m! macro
            if let Ok(updated) = AbsLBConst::new(num_per_m!(new_val)) {
                *obj = updated;
            }
        }
    }
}
