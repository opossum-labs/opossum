use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{Proptype, SplittingConfig};

use crate::components::node_editor::inputs::{input_components::{LabeledSelect, RowedInputs}, InputData, InputParam};

#[component]
pub fn SplitterTypeEditor (splitting_config: SplittingConfig, property_key: String, prop_type_sig: Signal<Proptype> ) -> Element{
    let select_id = format!("splitterTypeProperty{property_key}").to_camel_case();
    rsx! {
        LabeledSelect {
            id: select_id,
            label: "Splitting configuration",
            options: get_splitter_type_options(splitting_config),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(ref_ind_type) = SplittingConfig::default_from_name(val.as_str()) {
                    prop_type_sig.set(ref_ind_type.into());
                }
            },
        }
        div { class: "accordion-content-wrapper-div border-start",
            RowedInputs { inputs: get_splitting_config_input_data(splitting_config, prop_type_sig) }
        }
    }
}

fn get_splitting_config_input_data(splitting_config: &SplittingConfig, prop_type_sig: Signal<Proptype>,) -> Vec<InputData>{
    match splitting_config{
        SplittingConfig::Ratio(ratio) => vec![
            InputData::new(
                InputParam::F,
                &"splittingConfigRatioInput".to_string(),
                on_splitting_config_change(splitting_config, prop_type_sig, InputParam::RefractiveIndex),
                format!("{}", ref_ind.refractive_index()),
            )
        ],
        SplittingConfig::Spectrum(spectrum) => todo!(),
    }
}

fn get_splitter_type_options(splitting_config_type: SplittingConfig) -> Vec<(bool, String)>{
    let mut options = Vec::<(bool, String)>::new();

    for split_config in SplittingConfig::iter() {
        if std::mem::discriminant(&split_config) == std::mem::discriminant(splitting_config_type) {
            options.push((true, format!("{split_config}")));
        } else {
            options.push((false, format!("{split_config}")));
        }
    }
    options
}