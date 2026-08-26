use dioxus::prelude::*;
use opossum_core::absorption::{
    absorption_lb_constant::AbsLBConst,
    absorption_model::AbsorptionModel,
};
use crate::components::node_editor::inputs::InputData;

/// Parameter helper for the constant Lambert-Beer absorption model.
pub struct LambertBeerConstParam;

impl LambertBeerConstParam {
    pub fn to_input_data_vec(
        _model: &AbsLBConst,
        _on_save: EventHandler<AbsorptionModel>,
        _readonly: bool,
    ) -> Vec<InputData> {
        // TODO: Implement alpha input field
        Vec::new()
    }
}