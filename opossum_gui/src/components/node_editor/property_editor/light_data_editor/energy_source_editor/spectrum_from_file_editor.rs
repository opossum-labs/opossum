use std::path::PathBuf;

use crate::components::node_editor::inputs::{
    input_components::InputParamLabeledInput, InputParam, IntoInputData, IntoInputDataStrings,
};
use dioxus::prelude::*;
use opossum_backend::energy_data_builder::EnergyDataBuilder;
use strum::EnumIter;

#[component]
pub fn SpectrumFromFileEditor(
    path_buf: PathBuf,
    energy_data_builder_sig: Signal<EnergyDataBuilder>,
) -> Element {
    let input_data = IntoInputData::<String, PathBuf, EnergyDataBuilder>::to_input_data(
        &EnergySpectrumFromFileParam::FPath,
        path_buf,
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

impl IntoInputDataStrings<PathBuf> for EnergySpectrumFromFileParam {
    fn create_id_string(&self) -> String {
        "rayTypeEnergySrcfromFileInput".to_string()
    }
    fn create_value_string(&self, obj: &PathBuf) -> String {
        obj.file_name()
            .map_or("no file selected", |f| {
                f.to_str().unwrap_or("no file selected")
            })
            .to_string()
    }
}

impl IntoInputData<String, PathBuf, EnergyDataBuilder> for EnergySpectrumFromFileParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<String> {
        e.files().and_then(|file_engine| {
            let files = file_engine.files();
            if files.is_empty() {
                None
            } else {
                Some(files[0].clone())
            }
        })
    }
    fn setter_from_obj(&self) -> impl FnMut(&mut PathBuf, String) {
        move |obj: &mut PathBuf, val: String| *obj = PathBuf::from(val)
    }
}
