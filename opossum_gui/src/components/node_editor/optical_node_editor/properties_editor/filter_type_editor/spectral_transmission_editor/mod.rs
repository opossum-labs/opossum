mod band_filter_editor;
mod edge_filter_editor;

use std::path::Path;

use band_filter_editor::BandFilterEditor;
use edge_filter_editor::EdgeFilterEditor;

use dioxus::prelude::*;
use opossum_core::{prelude::SpectralFilterBuilder, utils::default_from_name::DefaultFromName};
use strum::EnumIter;

use crate::components::node_editor::{
    accordion::ElementList,
    hooks::use_update_signal_with_reactive_prop,
    inputs::{
        InputParam, IntoInputData, IntoInputDataStrings,
        input_components::{InputParamLabeledInput, LabeledSelect},
        select_options_from_enum_iterator,
    },
};

#[component]
pub fn SpectralFilterTypeSelector(
    spectral_filter_builder_sig: ReadSignal<SpectralFilterBuilder>,
    on_spectral_filter_change: EventHandler<SpectralFilterBuilder>,
) -> Element {
    rsx! {
        LabeledSelect {
            id: "nodeSpectralFilterBuilderSelector",
            label: "Spectral filter type",
            options: select_options_from_enum_iterator(&*spectral_filter_builder_sig.read(), None),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(sfb) = SpectralFilterBuilder::default_from_name(val.as_str()) {
                    on_spectral_filter_change.call(sfb);
                }
            },
        }
    }
}

#[component]
pub fn SpectralFilterTypeEditor<T: From<SpectralFilterBuilder> + PartialEq + Clone+ 'static>(
    spectral_filter_builder: SpectralFilterBuilder,
    on_spectral_filter_change: EventHandler<T>,
) -> Element {
    let mut spectral_filter_builder_sig = use_signal(|| spectral_filter_builder.clone());

    let on_spectral_filter_change = EventHandler::new(move |new_builder: SpectralFilterBuilder| {
        println!("New spectral filter builder");
        if new_builder != *spectral_filter_builder_sig.read() {
            println!("differing spectral filter builder");

            on_spectral_filter_change.call(new_builder.clone().into());
            spectral_filter_builder_sig.set(new_builder);
        }
     });

    let mut element_list = vec![rsx! {
    SpectralFilterTypeSelector {spectral_filter_builder_sig , on_spectral_filter_change}}];

    let editor = match spectral_filter_builder_sig() {
        SpectralFilterBuilder::EdgeFilter(edge_filter) => rsx! {
            EdgeFilterEditor { edge_filter, on_spectral_filter_change }
        },
        SpectralFilterBuilder::BandFilter(band_filter) => rsx! {
            BandFilterEditor { band_filter, on_spectral_filter_change }
        },
        SpectralFilterBuilder::FromFile(_) => 
        {
            let input_data = FilterFromFileParam::FPath.to_input_data(
                spectral_filter_builder_sig.read().clone(),
                on_spectral_filter_change,
            );
            rsx! {
                InputParamLabeledInput { input_data }
            }
        }
    };

    element_list.push(editor);

    rsx! {
        ElementList { element_list }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum FilterFromFileParam {
    FPath,
}

impl From<FilterFromFileParam> for InputParam {
    fn from(_: FilterFromFileParam) -> Self {
        Self::FilePath("File:".into(), ".csv".into())
    }
}

impl IntoInputDataStrings<SpectralFilterBuilder> for FilterFromFileParam {
    fn create_id_string(&self) -> String {
        "spectralFilterParamFilePathInput".to_string()
    }
    fn create_value_string(&self, obj: &SpectralFilterBuilder) -> String {
        obj.file_path().map_or_else(
            || "no file selected".to_string(),
            |fpath| {
                fpath
                    .file_name()
                    .map_or("no file selected", |f| {
                        f.to_str().unwrap_or("no file selected")
                    })
                    .to_string()
            },
        )
    }
}

impl IntoInputData<String, SpectralFilterBuilder, SpectralFilterBuilder> for FilterFromFileParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<String> {
        if e.value().is_empty() {
            None
        } else {
            Some(e.value())
        }
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut SpectralFilterBuilder, String) {
        move |obj: &mut SpectralFilterBuilder, val: String| {
            *obj = SpectralFilterBuilder::FromFile(Path::new(&val).to_path_buf());
        }
    }
}
