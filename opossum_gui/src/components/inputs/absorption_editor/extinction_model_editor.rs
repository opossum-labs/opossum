use dioxus::prelude::*;
use opossum_core::absorption::absorption_model::AbsorptionModel;
use crate::components::node_editor::inputs::InputData;

/// Parameter helper for the extinction coefficient (k) model.
pub struct ExtinctionParam;

impl ExtinctionParam {
    pub fn to_input_data_vec(
        _k: f64,
        _on_save: EventHandler<AbsorptionModel>,
        _readonly: bool,
    ) -> Vec<InputData> {
        // TODO: Implement extinction coefficient k input field
        Vec::new()
    }
}