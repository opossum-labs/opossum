#![allow(clippy::derive_partial_eq_without_eq)]

use crate::components::node_editor::{
    CallbackWrapper,
    inputs::{InputData, InputParam},
};
use dioxus::prelude::*;
use itertools::Itertools;

#[component]
pub fn LabeledCheckboxInput(
    id: String,
    label: String,
    value: String,
    onchange: CallbackWrapper,
    #[props(default = false)] readonly: bool,
) -> Element {
    rsx! {
        div {
            class: "form-floating-checkbox border-start",
            "data-mdb-input-init": "",
            label { class: "text-secondary", r#for: id.clone(), "{label}" }
            br {}
            input {
                class: "form-check-input text-light",
                id: id.clone(),
                name: id.as_str(),
                value: value.clone(),
                r#type: "checkbox",
                checked: value.parse::<bool>().unwrap_or_default(),
                onchange: move |e: Event<FormData>| onchange.call(e),
            }
        }
    }
}

#[component]
pub fn LabeledFileInput(
    id: String,
    label: String,
    value: String,
    onchange: CallbackWrapper,
    accept: String,
    #[props(default = false)] readonly: bool,
) -> Element {
    let id_click = id.clone();
    rsx! {
        div {
            class: "form-file border-start file-selection-wrapper",
            "data-mdb-input-init": "",
            input {
                class: "form-input text-light",
                id: id.clone(),
                r#type: "text",
                readonly: true,
                style: "cursor: pointer;",
                value: "{value}",
                onchange: move |e| onchange.call(e),
                onclick: move |_| {
                    if readonly { return; }
                    let accept_filter = accept.clone();
                    let id_target = id_click.clone();
                    spawn(async move {
                        let mut dialog = rfd::AsyncFileDialog::new()
                            .set_title("Select File");
                        if !accept_filter.is_empty() {
                            let ext = accept_filter.trim_start_matches('.');
                            dialog = dialog.add_filter("File Type", &[ext]);
                        }
                        if let Some(handle) = dialog.pick_file().await {
                            let path = handle.path().to_string_lossy().to_string();
                            let safe_path = path.replace("\\", "\\\\");
                            let js = format!(r#"
                                let el = document.getElementById('{id_target}');
                                if (el) {{
                                    el.value = '{safe_path}';
                                    el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                                }}
                            "#);
                            dioxus::document::eval(&js);
                        }
                    });
                }
            }
            label { class: "text-secondary", r#for: id, "{label}" }
        }
    }
}

#[component]
pub fn InputParamLabeledInput(input_data: InputData) -> Element {
    if let InputParam::Bool(label) = input_data.input_param {
        rsx! {
            LabeledCheckboxInput {
                id: input_data.id,
                label,
                value: input_data.value,
                onchange: input_data.callback_opt,
            }
        }
    } else if let InputParam::FilePath(label, accept) = input_data.input_param {
        rsx! {
            LabeledFileInput {
                id: input_data.id,
                label,
                value: input_data.value,
                onchange: input_data.callback_opt,
                accept,
            }
        }
    } else if let InputParam::Selection(label, options) = input_data.input_param {
        rsx! {
            LabeledSelect {
                id: input_data.id,
                label,
                options,
                onchange: move |e| input_data.callback_opt.call(e),
            }
        }
    } else {
        rsx! {
            LabeledInput {
                id: input_data.id,
                label: input_data.input_param.label(),
                value: input_data.value,
                onchange: input_data.callback_opt,
                r#type: input_data.input_param.rtype(),
                readonly: input_data.readonly,
            }
        }
    }
}

#[component]
pub fn RowedInputs(inputs: Vec<InputData>) -> Element {
    rsx! {
        for chunk in inputs.iter().chunks(2) {
            {
                let inputs: Vec<&InputData> = chunk.collect::<Vec<&InputData>>();
                if inputs.len() == 2 {
                    rsx! {
                        div { class: "row gy-1 gx-2",
                            div { class: "col-sm",
                                InputParamLabeledInput { input_data: inputs[0].clone() }
                            }
                            div { class: "col-sm",
                                InputParamLabeledInput { input_data: inputs[1].clone() }
                            }
                        }
                    }
                } else if inputs.len() == 1 {
                    rsx! {
                        InputParamLabeledInput { input_data: inputs[0].clone() }
                    }
                } else {
                    rsx! {}
                }
            }
        }
    }
}

#[component]
pub fn LabeledInput(
    id: String,
    label: String,
    value: String,
    onchange: CallbackWrapper,
    #[props(default = "text")] r#type: &'static str,
    #[props(optional)] step: Option<&'static str>,
    #[props(optional)] min: Option<&'static str>,
    #[props(optional)] max: Option<&'static str>,
    #[props(default = false)] readonly: bool,
) -> Element {
    rsx! {
        div { class: "form-floating border-start", "data-mdb-input-init": "",
            input {
                class: "form-control bg-dark text-light form-control-sm",
                id: id.as_str(),
                name: id.as_str(),
                placeholder: label,
                value,
                readonly,
                disabled: readonly,
                r#type,
                step: step.unwrap_or_default(),
                min: min.unwrap_or_default(),
                max: max.unwrap_or_default(),
                onchange: move |e: Event<FormData>| onchange.call(e),
            }
            label { class: "form-label text-secondary", r#for: id, "{label}" }
        }
    }
}

#[component]
pub fn LabeledSelect(
    id: String,
    label: String,
    options: Vec<(bool, String)>,
    onchange: Callback<Event<FormData>>,
) -> Element {
    rsx! {
        div { class: "form-floating border-start", "data-mdb-input-init": "",
            select {
                class: "form-select bg-dark text-light",
                id: id.as_str(),
                "aria-label": label,
                onchange,
                for (is_selected , option) in options {
                    option { selected: is_selected, value: option, {option.clone()} }
                }
            }
            label { class: "text-secondary", r#for: id, "{label}" }
        }
    }
}
