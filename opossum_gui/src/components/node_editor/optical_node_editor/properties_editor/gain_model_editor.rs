use crate::components::{
    logger::LogResultExt,
    node_editor::{
        hooks::use_synced_signal,
        inputs::{
            InputData, InputParam, IntoInputData, IntoInputDataStrings,
            input_components::{LabeledSelect, RowedInputs},
            select_options_from_enum_iterator,
        },
        node_config_editor::NodeChangeEvent,
        optical_node_editor::properties_editor::on_save_proptype_handler,
    },
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::{
    gain::{ConstGain, GainModel},
    utils::default_from_name::DefaultFromName,
};
use strum::EnumIter;
use uuid::Uuid;

/// Editor for a node's `amp config` property: a model selector plus the selected model's parameters.
///
/// The dropdown-plus-parameter-rows composition is the one `RefractiveIndexEditor` uses - both edit
/// an enum whose variants carry different parameter sets, and both save through the shared
/// [`on_save_proptype_handler`]. That the amplification model is additionally mirrored onto the
/// canvas is handled once in `OpticalNodeEditor`, so this editor stays an ordinary property editor.
#[component]
pub fn GainModelEditor(
    node_id: Memo<Uuid>,
    gain_model: GainModel,
    property_key: String,
    readonly: bool,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let gain_model_sig = use_synced_signal(gain_model);

    let on_save = on_save_proptype_handler(
        gain_model_sig,
        property_key.clone(),
        on_change,
        node_id.into(),
    );

    rsx! {
        LabeledSelect {
            id: format!("gainModelProperty{property_key}").to_camel_case(),
            label: "Amplification model",
            options: select_options_from_enum_iterator(&*gain_model_sig.read(), None),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(gain_model) = GainModel::default_from_name(val.as_str()) {
                    on_save.call(gain_model);
                }
            },
        }
        div { class: "accordion-content-wrapper-div border-start",
            RowedInputs { inputs: gain_model_input_data(gain_model_sig.into(), on_save, readonly) }
        }
    }
}

/// Returns the parameter rows of the currently selected model.
///
/// `GainModel::None` has no parameters, and neither has a variant that this GUI does not know yet
/// (the enum is `#[non_exhaustive]`) - both simply show the selector alone.
fn gain_model_input_data(
    gain_model_sig: ReadSignal<GainModel>,
    on_save: EventHandler<GainModel>,
    readonly: bool,
) -> Vec<InputData> {
    match &*gain_model_sig.read() {
        GainModel::Const(const_gain) => {
            ConstGainParam::to_input_data_vec(const_gain, on_save, readonly)
        }
        _ => Vec::new(),
    }
}

/// The single parameter of [`GainModel::Const`].
#[derive(Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum ConstGainParam {
    Gain,
}

impl From<ConstGainParam> for InputParam {
    fn from(_: ConstGainParam) -> Self {
        Self::F64("Gain factor".into())
    }
}

impl IntoInputDataStrings<ConstGain> for ConstGainParam {
    fn create_id_string(&self) -> String {
        "gainModelConstGainInput".to_string()
    }
    fn create_value_string(&self, obj: &ConstGain) -> String {
        format!("{:.3e}", obj.gain())
    }
}

impl IntoInputData<f64, ConstGain, GainModel> for ConstGainParam {
    fn setter_from_obj(&self) -> impl FnMut(&mut ConstGain, f64) {
        move |obj: &mut ConstGain, val: f64| {
            obj.set_gain(val)
                .log_err_with_context("validation failed in `set_gain` of ConstGain");
        }
    }
}
