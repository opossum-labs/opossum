mod air_model_editor;
mod conrady_model_editor;
mod const_model_editor;
mod schott_model_editor;
mod sellmeier1_model_editor;

use conrady_model_editor::ConradyParam;
use const_model_editor::ConstRefParam;
use schott_model_editor::SchottParam;
use sellmeier1_model_editor::Sellmeier1Param;
use air_model_editor::AirParam;

use dioxus::prelude::*;
use opossum_core::{
    refractive_index::RefractiveIndexType, 
    utils::default_from_name::DefaultFromName,
};

// Assuming these input components exist in your shared UI library
use crate::components::node_editor::inputs::{
    InputData, IntoInputData,
    input_components::{LabeledSelect, RowedInputs},
    select_options_from_enum_iterator,
};

/// Properties for the generalized `RefractiveIndexEditor`.
/// Follows the "Props down, Events up" pattern.
#[derive(Props, Clone, PartialEq)]
pub struct RefractiveIndexEditorProps {
    /// Read-only signal containing the current refractive index model.
    pub ref_ind_type: ReadSignal<RefractiveIndexType>,
    
    /// Event handler triggered when the model type or any parameter changes.
    pub on_change: EventHandler<RefractiveIndexType>,

    /// Base ID used for HTML element IDs to avoid conflicts.
    #[props(default = "refractiveIndex".to_string())]
    pub base_id: String,
    
    /// If true, disables all input fields and dropdowns.
    #[props(default = false)]
    pub readonly: bool,
}

/// A generic editor component for optical refractive index models.
#[component]
pub fn RefractiveIndexEditor(props: RefractiveIndexEditorProps) -> Element {
    // Read the current state from the signal
    let current_type = props.ref_ind_type.read();

    rsx! {
      div { class: "refractive-index-editor-container",
        // Dropdown to select the model type (e.g., Const, Sellmeier, etc.)
        LabeledSelect {
          id: format!("{}Select", props.base_id),
          label: "Refractive Index Definition",
          // Generate dropdown options based on the enum variants
          options: select_options_from_enum_iterator(&*current_type, None),
          readonly: props.readonly,
          onchange: move |e: Event<FormData>| {
              let val = e.value();
              // Instantiate the default variant based on the selection
              if let Some(new_ref_ind_type) = RefractiveIndexType::default_from_name(
                  val.as_str(),
              ) {
                  props.on_change.call(new_ref_ind_type);
              }
          },
        }

        // Dynamic input fields based on the currently selected model
        div { class: "accordion-content-wrapper-div border-start mt-2 px-2",
          RowedInputs { inputs: get_refractive_index_input_data(props.ref_ind_type, props.on_change, props.readonly) }
        }
      }
    }
}

/// Helper function to delegate input generation to the specific model editors.
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