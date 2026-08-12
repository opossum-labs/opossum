mod air_model_editor;
mod conrady_model_editor;
mod const_model_editor;
mod schott_model_editor;
mod sellmeier1_model_editor;

use conrady_model_editor::ConradyParam;
use const_model_editor::ConstRefParam;
use opossum_core::{
    refractive_index::RefractiveIndexType, utils::default_from_name::DefaultFromName,
};
use schott_model_editor::SchottParam;
use sellmeier1_model_editor::Sellmeier1Param;

use dioxus::prelude::*;
use inflector::Inflector;

use crate::components::node_editor::{
    hooks::use_synced_signal,
    inputs::{
        InputData, IntoInputData,
        input_components::{LabeledSelect, RowedInputs},
        select_options_from_enum_iterator,
    },
    optical_node_editor::properties_editor::refractive_index_editor::air_model_editor::AirParam,
};

/// The model selector plus the selected model's parameter rows for a refractive index n(λ).
///
/// This is the index editor itself, not a property editor: it does not know which property it edits
/// and reports every change through `on_save` instead. That is what lets it be embedded in a
/// [`MaterialEditor`](super::material_editor::MaterialEditor), which carries the index as one datum
/// of a whole material rather than as a property of its own.
///
/// # Arguments
///
/// * `id` - DOM id of the model selector, used to tie its label to it.
/// * `ref_ind_type` - the index model to show.
/// * `on_save` - called with the new model whenever the selection or a parameter changes.
/// * `readonly` - whether the inputs are shown read-only.
#[component]
pub fn RefractiveIndexEditor(
    id: String,
    ref_ind_type: RefractiveIndexType,
    on_save: EventHandler<RefractiveIndexType>,
    readonly: bool,
) -> Element {
    let ref_ind_type_sig = use_synced_signal(ref_ind_type);

    rsx! {
        LabeledSelect {
            id: id.to_camel_case(),
            label: "Refractive index definition",
            options: select_options_from_enum_iterator(&*ref_ind_type_sig.read(), None),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(ref_ind_type) = RefractiveIndexType::default_from_name(
                    val.as_str(),
                ) {
                    on_save.call(ref_ind_type);
                }
            },
        }
        div { class: "accordion-content-wrapper-div border-start",
            RowedInputs { inputs: get_refractive_index_input_data(ref_ind_type_sig.into(), on_save, readonly) }
        }
    }
}

fn get_refractive_index_input_data(
    ref_ind_type_sig: ReadSignal<RefractiveIndexType>,
    on_save: EventHandler<RefractiveIndexType>,
    readonly: bool,
) -> Vec<InputData> {
    match &*ref_ind_type_sig.read() {
        RefractiveIndexType::Const(ref_ind) => {
            ConstRefParam::to_input_data_vec(ref_ind, on_save, readonly)
        }
        RefractiveIndexType::Sellmeier1(ref_ind) => {
            Sellmeier1Param::to_input_data_vec(ref_ind, on_save, readonly)
        }
        RefractiveIndexType::Schott(ref_ind) => {
            SchottParam::to_input_data_vec(ref_ind, on_save, readonly)
        }
        RefractiveIndexType::Conrady(ref_ind) => {
            ConradyParam::to_input_data_vec(ref_ind, on_save, readonly)
        }
        RefractiveIndexType::Air(ref_ind) => {
            AirParam::to_input_data_vec(ref_ind, on_save, readonly)
        }
    }
}
