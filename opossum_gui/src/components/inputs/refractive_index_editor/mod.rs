mod air_model_editor;
mod conrady_model_editor;
mod const_model_editor;
mod schott_model_editor;
mod sellmeier1_model_editor;

use air_model_editor::AirParam;
use conrady_model_editor::ConradyParam;
use const_model_editor::ConstRefParam;
use schott_model_editor::SchottParam;
use sellmeier1_model_editor::Sellmeier1Param;

use dioxus::prelude::*;
use opossum_core::{
    refractive_index::RefractiveIndexType, utils::default_from_name::DefaultFromName,
};

use crate::components::node_editor::inputs::{
    InputData, IntoInputData,
    input_components::{FormContext, LabeledSelect, RowedInputs},
    select_options_from_enum_iterator,
};

/// A generic editor component for optical refractive index models.
#[component]
pub fn RefractiveIndexEditor(
    /// Reactive start value (can be passed as a Signal or Memo).
    value: ReadSignal<RefractiveIndexType>,

    /// Event handler triggered when the model type or any parameter changes.
    on_change: EventHandler<RefractiveIndexType>,

    /// Base ID used for HTML element IDs to avoid conflicts.
    #[props(default = "refractiveIndex".to_string())]
    base_id: String,

    /// If true, disables all input fields and dropdowns.
    #[props(default = false)]
    readonly: bool,
) -> Element {
    info!("🔄 Render: RefractiveIndexEditor");

    // *** This is a hack to avoid crashes while using FlushedTextInput *****
    let flush_trigger = use_signal(|| 0usize);
    let dirty_count = use_signal(|| 0usize);
    use_context_provider(|| FormContext {
        flush_trigger,
        dirty_count,
    });
    // **********************************************************************

    // 1. Internal State: This makes the component fully decoupled and snappy.
    let mut internal_state = use_signal(|| value.read().clone());

    // 2. Reactive Sync: If the parent loads a completely different material,
    //    we sync the external value into our internal state.
    use_effect(move || {
        let ext_val = value.read();
        if *ext_val != *internal_state.read() {
            internal_state.set(ext_val.clone());
        }
    });

    // 3. Stable Callback for propagating internal updates
    let handle_internal_change = use_callback(move |new_type: RefractiveIndexType| {
        internal_state.set(new_type.clone());
        on_change.call(new_type);
    });

    // 4. Stable Callback for LabeledSelect dropdown selection changes
    let handle_select_change = use_callback(move |e: Event<FormData>| {
        let val = e.value();
        if let Some(new_ref_ind_type) = RefractiveIndexType::default_from_name(val.as_str()) {
            handle_internal_change.call(new_ref_ind_type);
        }
    });

    // 5. Memoize the dropdown options to prevent re-allocating when only coefficients change
    let select_options =
        use_memo(move || select_options_from_enum_iterator(&*internal_state.read(), None));

    // Read the current state for input generation
    let current_type = internal_state.read();

    rsx! {
        div { class: "refractive-index-editor-container",
            LabeledSelect {
                id: format!("{}Select", base_id),
                label: "Refractive Index Definition".to_string(),
                options: select_options.read().clone(),
                readonly,
                onchange: handle_select_change,
            }

            div { class: "accordion-content-wrapper-div border-start mt-2 px-2",
                RowedInputs { inputs: get_refractive_index_input_data(&current_type, handle_internal_change, readonly) }
            }
        }
    }
}

/// Helper function evaluating the inputs purely based on the borrowed type.
fn get_refractive_index_input_data(
    current_type: &RefractiveIndexType,
    on_save: EventHandler<RefractiveIndexType>,
    readonly: bool,
) -> Vec<InputData> {
    match current_type {
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
