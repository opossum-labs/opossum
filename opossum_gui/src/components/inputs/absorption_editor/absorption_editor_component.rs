use super::catalog_transmittance_editor::CatalogTransmittanceParam;
use super::constant_attenuation_model_editor::ConstantAttenuationParam;
use super::extinction_model_editor::ExtinctionParam;
use super::lambert_beer_const_model_editor::LambertBeerConstParam;
use super::lambert_beer_spec_model_editor::LambertBeerSpecParam;

use dioxus::prelude::*;
use opossum_core::{
    absorption::absorption_model::AbsorptionModel,
    utils::default_from_name::DefaultFromName,
};

use crate::components::node_editor::inputs::{
    InputData, IntoInputData,
    input_components::{FormContext, LabeledSelect, RowedInputs},
    select_options_from_enum_iterator,
};

/// A generic editor component for optical absorption models.
#[component]
pub fn AbsorptionEditor(
    /// Reactive start value (passed as Signal or Memo).
    value: ReadSignal<AbsorptionModel>,

    /// Event handler triggered when the model type or any parameter changes.
    on_change: EventHandler<AbsorptionModel>,

    /// Base ID used for HTML element IDs to avoid conflicts.
    #[props(default = "absorptionModel".to_string())]
    base_id: String,

    /// If true, disables all input fields and dropdowns.
    #[props(default = false)]
    readonly: bool,
) -> Element {
    info!("🔄 Render: AbsorptionEditor");

    let flush_trigger = use_signal(|| 0usize);
    let dirty_count = use_signal(|| 0usize);
    use_context_provider(|| FormContext {
        flush_trigger,
        dirty_count,
    });

    let mut internal_state = use_signal(|| value.read().clone());

    use_effect(move || {
        let ext_val = value.read();
        if *ext_val != *internal_state.read() {
            internal_state.set(ext_val.clone());
        }
    });

    let handle_internal_change = use_callback(move |new_model: AbsorptionModel| {
        internal_state.set(new_model.clone());
        on_change.call(new_model);
    });

    let handle_select_change = use_callback(move |e: Event<FormData>| {
        let val = e.value();
        if let Some(new_model) = AbsorptionModel::default_from_name(val.as_str()) {
            handle_internal_change.call(new_model);
        }
    });

    let select_options =
        use_memo(move || select_options_from_enum_iterator(&*internal_state.read(), None));

    let current_model = internal_state.read();

    rsx! {
      div { class: "absorption-editor-container",
        LabeledSelect {
          id: format!("{}Select", base_id),
          label: "Absorption Model Definition".to_string(),
          options: select_options.read().clone(),
          readonly,
          onchange: handle_select_change,
        }

        div { class: "accordion-content-wrapper-div border-start mt-2 px-2",
          RowedInputs { inputs: get_absorption_input_data(&current_model, handle_internal_change, readonly) }
        }
      }
    }
}

/// Helper function evaluating input fields based on the active absorption model.
fn get_absorption_input_data(
    current_model: &AbsorptionModel,
    on_save: EventHandler<AbsorptionModel>,
    readonly: bool,
) -> Vec<InputData> {
    match current_model {
        AbsorptionModel::None => Vec::new(),
        AbsorptionModel::ConstantAttenuation(abs_const) => {
            ConstantAttenuationParam::to_input_data_vec(abs_const, on_save, readonly)
        }
        AbsorptionModel::LambertBeerConstant(abs_lb_const) => {
            LambertBeerConstParam::to_input_data_vec(abs_lb_const, on_save, readonly)
        }
        AbsorptionModel::LambertBeerSpectrum(spectrum) => {
            LambertBeerSpecParam::to_input_data_vec(spectrum, on_save, readonly)
        }
        AbsorptionModel::CatalogTransmittance(abs_cat_trans) => {
            CatalogTransmittanceParam::to_input_data_vec(abs_cat_trans, on_save, readonly)
        }
        AbsorptionModel::ExtinctionCoefficient(k) => {
            ExtinctionParam::to_input_data_vec(*k, on_save, readonly)
        }
    }
}