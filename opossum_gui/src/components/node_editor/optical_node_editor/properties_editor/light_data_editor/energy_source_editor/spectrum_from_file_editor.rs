use std::path::PathBuf;

use crate::components::logger::LogResultExt;
use crate::components::node_editor::inputs::{
    InputParam, IntoInputData, IntoInputDataStrings, input_components::InputParamLabeledInput,
};
use dioxus::prelude::*;
use opossum_core::{lightdata::energy_data_builder::SpectrumFile, prelude::EnergyDataBuilder};
use strum::EnumIter;

#[component]
pub fn SpectrumFromFileEditor(
    spec_file: SpectrumFile,
    on_save: EventHandler<EnergyDataBuilder>,
) -> Element {
    let input_data = IntoInputData::<String, SpectrumFile, EnergyDataBuilder>::to_input_data(
        &EnergySpectrumFromFileParam::FPath,
        spec_file,
        on_save,
    );
    rsx! {
        InputParamLabeledInput { input_data }
    }
}

#[derive(Clone, Copy, EnumIter, Eq, PartialEq)]
enum EnergySpectrumFromFileParam {
    FPath,
}

impl From<EnergySpectrumFromFileParam> for InputParam {
    fn from(_: EnergySpectrumFromFileParam) -> Self {
        Self::FilePath("File:".into(), ".csv".into())
    }
}

impl IntoInputDataStrings<SpectrumFile> for EnergySpectrumFromFileParam {
    fn create_id_string(&self) -> String {
        "rayTypeEnergySrcfromFileInput".to_string()
    }
    fn create_value_string(&self, obj: &SpectrumFile) -> String {
        let path = obj.f_path();
        let path_str = path.to_string_lossy();
        if path_str.is_empty() {
            "no file selected".to_string()
        } else {
            path_str.to_string()
        }
    }
}

impl IntoInputData<String, SpectrumFile, EnergyDataBuilder> for EnergySpectrumFromFileParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<String> {
        // 1. First, check if there is a text value (by using the rfd file selector)
        let value = e.value();
        if !value.is_empty() {
            return Some(value);
        }
        // 2. Fallback: Check for standard browser files (if used elsewhere)
        let files = e.files();
        if !files.is_empty() {
            return Some(files[0].name());
        }
        None
    }
    fn setter_from_obj(&self) -> impl FnMut(&mut SpectrumFile, String) {
        move |obj: &mut SpectrumFile, val: String| {
            obj.set_f_path(PathBuf::from(val))
                .log_err_with_context("Validation failed in `set_f_path`");
        }
    }
}
