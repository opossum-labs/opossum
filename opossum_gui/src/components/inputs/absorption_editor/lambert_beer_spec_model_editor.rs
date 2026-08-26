use dioxus::prelude::*;
use opossum_core::{
    absorption::absorption_model::AbsorptionModel,
    light::Spectrum,
};
use crate::components::node_editor::inputs::InputData;

/// Parameter helper for the spectral Lambert-Beer absorption model.
pub struct LambertBeerSpecParam;

impl LambertBeerSpecParam {
    pub fn to_input_data_vec(
        _spectrum: &Spectrum,
        _on_save: EventHandler<AbsorptionModel>,
        _readonly: bool,
    ) -> Vec<InputData> {
        // TODO: Implement spectrum selection/editor integration
        Vec::new()
    }
}