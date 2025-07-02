#![allow(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;
use itertools::Itertools;

use crate::components::node_editor::{
    inputs::{InputData, InputParam},
    CallbackWrapper,
};

#[component]
pub fn DistLabeledInput(dist_input: InputData) -> Element {
    if dist_input.dist_param == InputParam::Rectangular {
        let label = dist_input.dist_param.input_label();
        rsx! {
            div {
                class: "form-floating-checkbox border-start",
                "data-mdb-input-init": "",
                label { class: "text-secondary", r#for: dist_input.id.clone(), "{label}" }
                br {}
                input {
                    class: "form-check-input text-light",
                    id: dist_input.id.as_str(),
                    name: dist_input.id.as_str(),
                    value: dist_input.value.clone(),
                    r#type: "checkbox",
                    role: "switch",
                    checked: dist_input.value.parse::<bool>().unwrap_or_default(),
                    onchange: move |e| dist_input.callback_opt.call(e),
                }
            }
        }
    } else if dist_input.dist_param == InputParam::FilePath {
        let label = format!(
            "{}: {}",
            dist_input.dist_param.input_label(),
            dist_input.value
        );
        rsx! {
            div {
                id: "imgSrcFileSelection",
                class: "form-file border-start",
                "data-mdb-input-init": "",
                input {
                    class: "form-input text-light",
                    id: dist_input.id.as_str(),
                    r#type: "file",
                    accept: ".png",
                    onchange: move |e| dist_input.callback_opt.call(e),
                }
                label {
                    id: "imgSrcFileSelectionLabel",
                    class: "btn bg-dark text-secondary",
                    r#for: dist_input.id,
                    "{label}"
                }
            }
        }
    } else {
        rsx! {
            LabeledInput {
                id: dist_input.id,
                label: dist_input.dist_param.input_label(),
                value: dist_input.value,
                step: dist_input.dist_param.step_value(),
                min: dist_input.dist_param.min_value(),
                onchange: dist_input.callback_opt,
                r#type: "number",
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
                                DistLabeledInput { dist_input: inputs[0].clone() }
                            }
                            div { class: "col-sm",
                                DistLabeledInput { dist_input: inputs[1].clone() }
                            }
                        }
                    }
                } else if inputs.len() == 1 {
                    rsx! {
                        DistLabeledInput { dist_input: inputs[0].clone() }
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

    #[props(default = false)] hidden: bool,

    #[props(default = false)] readonly: bool,
) -> Element {
    rsx! {
        div {
            class: "form-floating border-start",
            "data-mdb-input-init": "",
            hidden,
            input {
                class: "form-control bg-dark text-light form-control-sm",
                id: id.as_str(),
                name: id.as_str(),
                placeholder: label,
                value,
                readonly,
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
