use dioxus::prelude::*;
use opossum_core::absorption::{
    absorption_catalog_transmittance::AbsCatTrans,
    absorption_model::AbsorptionModel,
};
use crate::components::node_editor::inputs::InputData;

/// Parameter helper for tabulated catalog transmittance data.
pub struct CatalogTransmittanceParam;

impl CatalogTransmittanceParam {
    pub fn to_input_data_vec(
        _model: &AbsCatTrans,
        _on_save: EventHandler<AbsorptionModel>,
        _readonly: bool,
    ) -> Vec<InputData> {
        // TODO: Implement reference thickness input and data table
        Vec::new()
    }
}