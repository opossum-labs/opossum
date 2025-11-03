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
    energy_data_builder_sig: Signal<EnergyDataBuilder>,
) -> Element {
    let input_data = IntoInputData::<String, SpectrumFile, EnergyDataBuilder>::to_input_data(
        &EnergySpectrumFromFileParam::FPath,
        spec_file,
        energy_data_builder_sig,
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
        obj.f_path()
            .file_name()
            .map_or("no file selected", |f| {
                f.to_str().unwrap_or("no file selected")
            })
            .to_string()
    }
}

impl IntoInputData<String, SpectrumFile, EnergyDataBuilder> for EnergySpectrumFromFileParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<String> {
        if e.files().is_empty() {
            None
        } else {
            Some(e.files()[0].name().clone())
        }
    }
    fn setter_from_obj(&self) -> impl FnMut(&mut SpectrumFile, String) {
        move |obj: &mut SpectrumFile, val: String| {
            obj.set_f_path(PathBuf::from(val))
                .log_err_with_context("Validation failed in `set_f_path`");
        }
    }
}
