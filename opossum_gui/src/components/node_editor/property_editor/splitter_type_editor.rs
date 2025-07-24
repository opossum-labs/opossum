use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{Proptype, SplittingConfig, DefaultFromName};

use crate::components::node_editor::inputs::{input_components::{LabeledSelect, RowedInputs}, select_options_from_enum_iterator, InputData, InputParam};

#[component]
pub fn SplitterTypeEditor (splitting_config: SplittingConfig, property_key: String, prop_type_sig: Signal<Proptype> ) -> Element{
    let select_id = format!("splitterTypeProperty{property_key}").to_camel_case();
    rsx! {
        LabeledSelect {
            id: select_id,
            label: "Splitting configuration",
            options: select_options_from_enum_iterator(&splitting_config, None),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(splitting_type) = SplittingConfig::default_from_name(val.as_str()) {
                    prop_type_sig.set(splitting_type.into());
                }
            },
        }
    }
}

// #[derive(Clone, Copy, PartialEq, Eq, EnumIter)]
// enum SplittingConfigParam{
//     Ratio,
//     SpecFromFile,
//     SpecFromLongpass,
//     SpecFromShortpass
// }

// impl From<SplittingConfigParam> for InputParam {
//     fn from(value: SplittingConfigParam) -> Self {
//         match value {
//             SplittingConfigParam::Ratio => Self::F64("Ratio"),
//             SplittingConfigParam::Spectrum => Self::File("File:")
//         }
//     }
// }

// impl IntoInputDataStrings<SplittingConfig> for SplittingConfigParam {
//     fn create_id_string(&self) -> String {
//         let id_str = match self {
//             Self::Ratio => "Ratio",
//             Self::Spectrum => "SpecFromFile",
//         };

//         format!("SplittingConfig{id_str}Input")
//     }
//     fn create_value_string(&self, obj: &SplittingConfig) -> String {
//         match self {
//             Self::Ratio => {
//                 format!("{:.3e}", obj.wavelength_range().start.get::<nanometer>())
//             }
//             Self::Spectrum => format!("{:.3e}", obj.wavelength_range().end.get::<nanometer>()),
//         }
//     }
// }

// impl IntoInputData<f64, RefrIndexConrady, RefractiveIndexType> for ConradyParam {
//     fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
//         let e_value = e.value();
//         e_value.parse::<f64>().ok()
//     }

//     fn setter_from_obj(&self) -> impl FnMut(&mut RefrIndexConrady, f64) {
//         match self {
//             Self::WaveLengthStart => move |obj: &mut RefrIndexConrady, val: f64| {
//                 obj.set_wavelength_range_start(nanometer!(val));
//             },
//             Self::WavelengthEnd => move |obj: &mut RefrIndexConrady, val: f64| {
//                 obj.set_wavelength_range_end(nanometer!(val));
//             },
//             Self::A => move |obj: &mut RefrIndexConrady, val: f64| obj.set_n0(val),
//             Self::B => move |obj: &mut RefrIndexConrady, val: f64| obj.set_a(val),
//             Self::C => move |obj: &mut RefrIndexConrady, val: f64| obj.set_b(val),
//         }
//     }
// }


// fn get_splitting_config_input_data(splitting_config: &SplittingConfig, prop_type_sig: Signal<Proptype>,) -> Vec<InputData>{
//     match splitting_config{
//         SplittingConfig::Ratio(ratio) => vec![
//             InputData::new(
//                 InputParam::F,
//                 &"splittingConfigRatioInput".to_string(),
//                 on_splitting_config_change(splitting_config, prop_type_sig, InputParam::RefractiveIndex),
//                 format!("{}", ref_ind.refractive_index()),
//             )
//         ],
//         SplittingConfig::Spectrum(spectrum) => todo!(),
//     }
// }

// fn get_splitter_type_options(splitting_config_type: SplittingConfig) -> Vec<(bool, String)>{
//     let mut options = Vec::<(bool, String)>::new();

//     for split_config in SplittingConfig::iter() {
//         if std::mem::discriminant(&split_config) == std::mem::discriminant(splitting_config_type) {
//             options.push((true, format!("{split_config}")));
//         } else {
//             options.push((false, format!("{split_config}")));
//         }
//     }
//     options
// }