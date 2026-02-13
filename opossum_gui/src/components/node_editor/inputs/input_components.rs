#![allow(clippy::derive_partial_eq_without_eq)]

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::inputs::{
        InputData, InputParam, format_exp_number_notation, format_si_with_base_unit,
        is_permissive_exp_input, is_permissive_unit_input, parse_exp_input_strict, parse_si_number,
        parse_unit_input_strict,
    },
};
use dioxus::prelude::*;
use itertools::Itertools;
use std::ops::{AddAssign, SubAssign};

// ========================================================
// 1. NEU: SEMAPHORE PROTOKOLL (Dirty Check)
// ========================================================

#[derive(Clone, Copy)]
pub struct FormContext {
    pub flush_trigger: Signal<usize>,
    pub dirty_count: Signal<usize>,
}

#[component]
pub fn FlushableTextInput(
    id: String,
    label: String,
    value: ReadSignal<String>,
    on_save: EventHandler<String>,
    #[props(default = String::new())] container_class: String,
    #[props(default = String::new())] input_class: String,
    #[props(default = String::new())] label_class: String,
    #[props(default = "text")] r#type: &'static str,
    #[props(optional)] step: Option<&'static str>,
    #[props(optional)] min: Option<&'static str>,
    #[props(optional)] max: Option<&'static str>,
    #[props(optional)] eval_input: Option<Callback<String, bool>>,
    #[props(default = false)] readonly: bool,
) -> Element {
    let mut form_ctx = use_context::<FormContext>();

    let mut local_value = use_signal(|| value.read().clone());
    let mut is_locally_dirty = use_signal(|| false);

    use_effect(use_reactive!(|value| {
        local_value.set(value.read().clone());
        is_locally_dirty.set(false);
    }));

    let mut perform_save = move || {
        if *is_locally_dirty.peek() {
            let val = local_value.peek().clone();
            on_save.call(val);
            is_locally_dirty.set(false);
            form_ctx.dirty_count.write().sub_assign(1);
        }
    };

    let flush_sig = form_ctx.flush_trigger;
    use_effect(move || {
        flush_sig();
        perform_save();
    });

    rsx! {
        div { class: container_class, "data-mdb-input-init": "",
            input {
                class: input_class,
                id: id.as_str(),
                name: id.as_str(),
                placeholder: label,
                value: local_value.read().clone(),
                readonly,
                disabled: readonly,
                r#type,
                step: step.unwrap_or_default(),
                min: min.unwrap_or_default(),
                max: max.unwrap_or_default(),

                oninput: move |e: Event<FormData>| {
                    let new_value = e.data.value();
                    if let Some(eval_input) = eval_input {
                        if eval_input(new_value.clone()) {
                            local_value.set(new_value);
                            if !*is_locally_dirty.peek() {
                                is_locally_dirty.set(true);
                                form_ctx.dirty_count.write().add_assign(1);
                            }
                        } else {
                            local_value.set(local_value());
                        }
                    } else {
                        local_value.set(new_value);
                        if !*is_locally_dirty.peek() {
                            is_locally_dirty.set(true);
                            form_ctx.dirty_count.write().add_assign(1);
                        }
                    }
                },
                onblur: move |_| perform_save(),
                onkeydown: move |e: Event<KeyboardData>| {
                    if e.key() == Key::Enter {
                        perform_save();
                    }
                },
            }
            label { class: label_class, r#for: id,
                "{label}"
                if *is_locally_dirty.read() {
                    span { class: "text-warning", " *" }
                }
            }
        }
    }
}

// ========================================================
// 2. EXISTIERENDE KOMPONENTEN (Wiederhergestellt)
// ========================================================

#[component]
pub fn LabeledCheckboxInput(
    id: String,
    label: String,
    value: String,
    onchange: EventHandler<Event<FormData>>,
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
    onchange: EventHandler<Event<FormData>>,
    accept: String,
    #[props(default = false)] readonly: bool,
) -> Element {
    let id_target = id.clone();
    let onchange_input = onchange;

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
                        if readonly {
                            return;
                        }
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
                                let js = format!(
                                    r#"let el=document.getElementById('{target_id}');if (el) {{let nativeInputValueSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;nativeInputValueSetter.call(el, '{safe_path}');el.dispatchEvent(new Event('input', {{ bubbles: true }}));el.dispatchEvent(new Event('change', {{ bubbles: true }}));}}"#,
                                );
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
                onchange: input_data.callback,
            }
        }
    } else if let InputParam::FilePath(label, accept) = input_data.input_param {
        rsx! {
            LabeledFileInput {
                id: input_data.id,
                label,
                value: input_data.value,
                onchange: input_data.callback,
                accept,
            }
        }
    } else if let InputParam::Selection(label, options) = input_data.input_param {
        rsx! {
            LabeledSelect {
                id: input_data.id,
                label,
                options,
                onchange: move |e| input_data.callback.call(e),
            }
        }
    } else if let InputParam::SIUnit(label, base_unit) = input_data.input_param {
        rsx! {
            NodeConfigUnitInput {
                id: input_data.id,
                label,
                value: input_data.value.parse::<f64>().unwrap_or_default(),
                base_unit,
                onchange: move |new_value: f64| {
                    input_data.callback_str.call(new_value.to_string());
                },
                readonly: input_data.readonly,
            }
        }
    } else if let InputParam::F64(label) = input_data.input_param {
        rsx! {
            NodeConfigPlainF64Input {
                id: input_data.id,
                label,
                value: input_data.value.parse::<f64>().unwrap_or_default(),
                onchange: move |new_value: f64| {
                    input_data.callback_str.call(new_value.to_string());
                },
                readonly: input_data.readonly,
            }
        }
    } else {
        rsx! {
            FlushableTextInput {
                id: input_data.id,
                label: input_data.input_param.label(),
                value: input_data.value,
                on_save: input_data.callback_str,
                r#type: input_data.input_param.rtype(),
                readonly: input_data.readonly,
                container_class: "form-floating border-start".to_string(),
                input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
                label_class: "form-label text-secondary".to_string(),
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
pub fn RowedElements(elements: Vec<Element>, num_per_row: usize) -> Element {
    rsx! {
        for chunk in elements.iter().chunks(num_per_row) {
            {
                let elements_in_row: Vec<&Element> = chunk.collect::<Vec<&Element>>();
                {
                    rsx! {
                        div { class: "row gy-1 gx-2",
                            for elem in elements_in_row {
                                div { class: "col-sm", {elem.clone()} }
                            }
                        }
                    }
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
    onchange: EventHandler<Event<FormData>>,
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
pub fn NodeConfigUnitInput(
    id: String,
    label: String,
    value: ReadSignal<f64>,
    base_unit: String,
    onchange: EventHandler<f64>,
    #[props(default = false)] reciprocal: bool,
    #[props(default = false)] readonly: bool,
) -> Element {
    rsx! {
        UnitInput {
            id,
            label,
            value,
            base_unit,
            onchange,
            container_class: "form-floating border-start".to_string(),
            input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
            label_class: "form-label text-secondary".to_string(),
            readonly,
            reciprocal,
            flushable_input: true,
        }
    }
}

#[component]
pub fn NodeConfigPlainF64Input(
    id: String,
    label: String,
    value: ReadSignal<f64>,
    onchange: EventHandler<f64>,
    #[props(default = false)] readonly: bool,
) -> Element {
    let mut val_str = use_signal(|| format_exp_number_notation(*value.read()));

    use_effect({
        move || {
            let current_str = val_str.peek().clone();
            let new_str = format_exp_number_notation(*value.read());
            if current_str != new_str {
                val_str.set(new_str);
            }
        }
    });

    let on_input_eval = { move |input_val: String| is_permissive_exp_input(&input_val) };

    let on_input_submission = EventHandler::new({
        move |val: String| {
            if let Ok(num_str) = parse_exp_input_strict(&val) {
                if let Ok(parsed) = num_str.parse::<f64>() {
                    onchange.call(parsed);
                    val_str.set(format_exp_number_notation(parsed));
                } else {
                    val_str.set(format_exp_number_notation(*value.read()));
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log("Cannot parse input number string to f64!");
                }
            }
        }
    });

    rsx! {
        FlushableTextInput {
            id,
            label,
            value: val_str,
            readonly,
            container_class: "form-floating border-start".to_string(),
            input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
            label_class: "form-label text-secondary".to_string(),
            eval_input: Some(Callback::new(on_input_eval)),
            on_save: on_input_submission,
        }
    }
}

#[component]
pub fn UnitInput(
    id: String,
    label: String,
    value: ReadSignal<f64>,
    base_unit: String,
    onchange: EventHandler<f64>,
    #[props(default = false)] reciprocal: bool,
    #[props(default = String::new())] container_class: String,
    #[props(default = String::new())] input_class: String,
    #[props(default = String::new())] label_class: String,
    #[props(default = false)] readonly: bool,
    #[props(default = false)] flushable_input: bool,
) -> Element {
    let mut val_str =
        use_signal(|| format_si_with_base_unit(*value.read(), &base_unit, reciprocal));

    let on_input_eval = { move |input_val: String| is_permissive_unit_input(&input_val) };

    let on_input_submission = EventHandler::new({
        let label = label.clone();
        move |val: String| {
            let old_val = val_str();
            if let Ok((num_str, prefix_str)) = parse_unit_input_strict(&val, &base_unit) {
                if let Some(parsed) = parse_si_number(&num_str, &prefix_str, reciprocal) {
                    val_str.set(format_si_with_base_unit(parsed, &base_unit, reciprocal));
                    onchange.call(parsed);
                } else {
                    val_str.set(old_val);
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log("Cannot parse input number string to f64!");
                }
            } else {
                val_str.set(old_val);
                OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Wrong input format for field `{label}`! Must have unit `{base_unit}` and a valid prefix."));
            }
        }
    });

    if flushable_input {
        rsx! {
            FlushableTextInput {
                id,
                label,
                value: val_str,
                readonly,
                container_class,
                input_class,
                label_class,
                eval_input: Some(Callback::new(on_input_eval)),
                on_save: on_input_submission,
            }
        }
    } else {
        rsx! {
            input {
                class: input_class,
                id,
                value: val_str,
                readonly,
                oninput: move |e: Event<FormData>| {
                    let new_value = e.data.value();
                    if !on_input_eval(new_value) {
                        val_str.set(val_str());
                    }
                },
                onchange: move |e: Event<FormData>| on_input_submission.call(e.data.value()),
            }
        }
    }
}

#[component]
pub fn LabeledSelect(
    id: String,
    label: String,
    options: Vec<(bool, String)>,
    onchange: EventHandler<Event<FormData>>,
) -> Element {
    rsx! {
        div { class: "form-floating border-start", "data-mdb-input-init": "",
            select {
                class: "form-select bg-dark text-light",
                id: id.as_str(),
                "aria-label": label,
                onchange: move |e| onchange.call(e),
                for (is_selected , option) in options {
                    option { selected: is_selected, value: option, {option.clone()} }
                }
            }
            label { class: "text-secondary", r#for: id, "{label}" }
        }
    }
}
