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
    inputs::{
        InputData, IntoInputData,
        input_components::{LabeledSelect, RowedInputs},
        select_options_from_enum_iterator,
    },
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::{
        refractive_index_editor::air_model_editor::AirParam, on_save_proptype_handler,
    },
};
use uuid::Uuid;

#[component]
pub fn RefractiveIndexEditor(
    node_id: Memo<Uuid>,
    ref_ind_type: RefractiveIndexType,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let ref_ind_type_sig = use_signal(|| ref_ind_type.clone());

    let on_save = on_save_proptype_handler(
        ref_ind_type_sig,
        property_key.clone(),
        on_change,
        node_id.into(),
    );

    rsx! {
        LabeledSelect {
            id: format!("refractiveIndexProperty{property_key}").to_camel_case(),
            label: "Refractive index definition",
            options: select_options_from_enum_iterator(&*ref_ind_type_sig.read(), None),
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
            RowedInputs { inputs: get_refractive_index_input_data(ref_ind_type_sig.into(), on_save) }
        }
    }
}

fn get_refractive_index_input_data(
    ref_ind_type_sig: ReadSignal<RefractiveIndexType>,
    on_save: EventHandler<RefractiveIndexType>,
) -> Vec<InputData> {
    match &*ref_ind_type_sig.read() {
        RefractiveIndexType::Const(ref_ind) => ConstRefParam::to_input_data_vec(ref_ind, on_save),
        RefractiveIndexType::Sellmeier1(ref_ind) => {
            Sellmeier1Param::to_input_data_vec(ref_ind, on_save)
        }
        RefractiveIndexType::Schott(ref_ind) => {
            SchottParam::to_input_data_vec(ref_ind, ref_ind_type_sig)
        }
        RefractiveIndexType::Conrady(ref_ind) => {
            ConradyParam::to_input_data_vec(ref_ind, ref_ind_type_sig)
        }
        RefractiveIndexType::Air(ref_ind) => AirParam::to_input_data_vec(ref_ind, ref_ind_type_sig),
    }
}
