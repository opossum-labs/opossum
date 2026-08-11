use crate::components::{
    logger::LogResultExt,
    node_editor::{
        hooks::use_synced_signal,
        inputs::{
            InputData, InputParam, IntoInputData, IntoInputDataStrings,
            input_components::{LabeledSelect, RowedInputs},
            select_options_from_enum_iterator,
        },
    },
    scenery_editor::GraphsWorkspaceAction,
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
/// an enum whose variants carry different parameter sets.
///
/// Unlike the other property editors this does **not** go through the generic
/// `NodeChangeAction::Property` path but sends [`GraphsWorkspaceAction::SetAmpConfig`], the same
/// action the canvas context menu sends. The amplification model is the one property that is also
/// displayed on the canvas, and routing both editing paths through one handler keeps that mirroring
/// in a single place instead of special-casing this property inside the generic one.
#[component]
pub fn GainModelEditor(
    node_id: Memo<Uuid>,
    graph_id: Memo<Uuid>,
    gain_model: GainModel,
    property_key: String,
    readonly: bool,
) -> Element {
    let mut gain_model_sig = use_synced_signal(gain_model);
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();

    let on_save = EventHandler::new(move |model: GainModel| {
        if model == *gain_model_sig.read() {
            return;
        }
        // Show the new value right away; the refetch triggered by the patch confirms it a moment
        // later, and without this the select would visibly snap back in between.
        gain_model_sig.set(model);
        workspace_processor.send(GraphsWorkspaceAction::SetAmpConfig {
            node_id: *node_id.read(),
            graph_id: *graph_id.read(),
            model,
        });
    });

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
