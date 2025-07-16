mod conrady_model_editor;
mod const_model_editor;
mod schott_model_editor;
mod sellmeier1_model_editor;

use conrady_model_editor::ConradyParam;
use const_model_editor::ConstRefParam;
use schott_model_editor::SchottParam;
use sellmeier1_model_editor::Sellmeier1Param;

use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{DefaultFromName, Proptype, RefractiveIndexType};

use crate::components::node_editor::inputs::{
    input_components::{LabeledSelect, RowedInputs},
    select_options_from_enum_iterator, InputData, IntoInputData,
};

#[component]
pub fn RefractiveIndexEditor(
    property_key: String,
    prop_type_sig: Signal<Proptype>,
    ref_ind_sig: Signal<RefractiveIndexType>,
) -> Element {
    use_effect(move || {
        prop_type_sig.set(ref_ind_sig.read().clone().into());
    });

    let select_id = format!("refractiveIndexProperty{property_key}").to_camel_case();
    rsx! {
        LabeledSelect {
            id: select_id,
            label: "Refractive index definition",
            options: select_options_from_enum_iterator(&*ref_ind_sig.read(), None),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(ref_ind_type) = RefractiveIndexType::default_from_name(
                    val.as_str(),
                ) {
                    ref_ind_sig.set(ref_ind_type);
                }
            },
        }
        div { class: "accordion-content-wrapper-div border-start",
            RowedInputs { inputs: get_refractive_index_input_data(&ref_ind_sig.read(), ref_ind_sig) }
        }
    }
}

fn get_refractive_index_input_data(
    ref_ind_type: &RefractiveIndexType,
    ref_ind_sig: Signal<RefractiveIndexType>,
) -> Vec<InputData> {
    match ref_ind_type {
        RefractiveIndexType::Const(ref_ind) => {
            ConstRefParam::to_input_data_vec(ref_ind, ref_ind_sig)
        }
        RefractiveIndexType::Sellmeier1(ref_ind) => {
            Sellmeier1Param::to_input_data_vec(ref_ind, ref_ind_sig)
        }
        RefractiveIndexType::Schott(ref_ind) => {
            SchottParam::to_input_data_vec(ref_ind, ref_ind_sig)
        }
        RefractiveIndexType::Conrady(ref_ind) => {
            ConradyParam::to_input_data_vec(ref_ind, ref_ind_sig)
        }
    }
}
