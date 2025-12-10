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
    let id_target = id.clone();
    let onchange_input = onchange.clone();
    rsx! {
        div { class: "form-file border-start file-selection-wrapper",
            div { class: "input-group",
                div { class: "form-floating",
                    input {
                        class: "form-control bg-dark text-light form-control-sm",
                        id: id.clone(),
                        r#type: "text",
                        value: "{value}",
                        readonly: true,
                        onchange: move |e| onchange.call(e),
                        oninput: move |e| onchange_input.call(e),
                    }
                    label { class: "text-secondary", r#for: id, "{label}" }
                }
                button {
                    class: "btn btn-secondary btn-sm",
                    r#type: "button",
                    disabled: readonly,
                    onclick: move |_| {
                        if readonly { return; }
                        let filter = accept.clone();
                        let target_id = id_target.clone();

                        spawn(async move {
                            let mut dialog = rfd::AsyncFileDialog::new().set_title("Select File");
                            if !filter.is_empty() {
                                let ext = filter.trim_start_matches('.');
                                dialog = dialog.add_filter("File Type", &[ext]);
                            }

                            if let Some(handle) = dialog.pick_file().await {
                                let path = handle.path().to_string_lossy().to_string();
                                let safe_path = path.replace('\\', "\\\\").replace('\'', "\\'");
                                let js = format!(r#"
                                    let el = document.getElementById('{target_id}');
                                    if (el) {{
                                        let nativeInputValueSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
                                        nativeInputValueSetter.call(el, '{safe_path}');
                                        el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                                        el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                                    }}
                                "#);
                                dioxus::document::eval(&js);
                            }
                        });
                    },
                    span { class: "fa-solid fa-folder-open", "" }
                    " Select"
                }
            }
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
