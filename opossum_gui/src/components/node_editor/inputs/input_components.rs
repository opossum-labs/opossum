use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::inputs::{
        InputData, InputParam, format_exp_number_notation, format_si_with_base_unit,
        parse_exp_input_strict, parse_si_number, parse_unit_input_permissive,
        parse_unit_input_strict,
    },
};
use dioxus::prelude::*;
use itertools::Itertools;
use opossum_core::opm_document::optimization::ParameterUnit;
use std::ops::{AddAssign, SubAssign};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FormContext {
    pub flush_trigger: Signal<usize>,
    pub dirty_count: Signal<usize>,
}

impl FormContext {
    /// Increments the count of fields with unsaved changes.
    pub fn mark_dirty(&mut self) {
        self.dirty_count.write().add_assign(1);
    }

    /// Decrements the count of fields with unsaved changes.
    pub fn mark_clean(&mut self) {
        self.dirty_count.write().sub_assign(1);
    }
}

#[component]
pub fn FlushableTextInput(
    id: String,
    label: String,
    value: String,
    on_save: EventHandler<String>,
    /// Explicitly supply a `FormContext`, or leave as None to automatically resolve via ambient context.
    #[props(optional)]
    form_context: Option<FormContext>,
    #[props(default = String::new())] container_class: String,
    #[props(default = String::new())] input_class: String,
    #[props(default = String::new())] label_class: String,
    #[props(default = "text")] r#type: &'static str,
    #[props(optional)] step: Option<&'static str>,
    #[props(optional)] min: Option<&'static str>,
    #[props(optional)] max: Option<&'static str>,
    #[props(default = false)] readonly: bool,
) -> Element {
    // Resolve context: explicit prop takes precedence; fall back to ambient context without panicking.
    let resolved_ctx = form_context.or_else(try_use_context::<FormContext>);

    let mut local_value = use_signal(|| value.clone());
    let mut is_locally_dirty = use_signal(|| false);

    // Tracks the prop's own last-seen value separately from `local_value` to allow optimistic updates.
    let mut last_prop_value = use_signal(|| value.clone());

    if *last_prop_value.peek() != value {
        last_prop_value.set(value.clone());
        if !*is_locally_dirty.peek() {
            local_value.set(value);
        }
    }

    let mut perform_save = move || {
        if *is_locally_dirty.peek() {
            let val = local_value.peek().clone();
            on_save.call(val);
            is_locally_dirty.set(false);
            if let Some(mut ctx) = resolved_ctx {
                ctx.mark_clean();
            }
        }
    };

    // Listen to external flush signals when a FormContext is available.
    use_effect(move || {
        if let Some(ctx) = resolved_ctx {
            // Subscribing to signal changes triggers this effect when flush_trigger updates.
            (ctx.flush_trigger)();
            perform_save();
        }
    });

    // Snap displayed text back to confirmed prop value when save completes.
    use_effect(move || {
        if !*is_locally_dirty.read() {
            local_value.set(last_prop_value.peek().clone());
        }
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
                    local_value.set(new_value);
                    if !*is_locally_dirty.peek() {
                        is_locally_dirty.set(true);
                        if let Some(mut ctx) = resolved_ctx {
                            ctx.mark_dirty();
                        }
                    }
                },
                onblur: move |_| perform_save(),
                onkeydown: move |e: Event<KeyboardData>| {
                    if e.key() == Key::Enter {
                        perform_save();
                    }
                    e.stop_propagation();
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
                disabled: readonly,
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
                                // Extract full file path on desktop targets
                                #[cfg(feature = "desktop")]
                                let selected_path = handle.path().to_string_lossy().to_string();

                                // Extract file name on web targets due to browser sandbox restrictions
                                #[cfg(not(feature = "desktop"))]
                                let selected_path = handle.file_name();

                                // Escape special characters for JS string injection
                                let safe_path = selected_path.replace('\\', "\\\\").replace('\'', "\\'");
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
                readonly: input_data.readonly,
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
                readonly: input_data.readonly,
            }
        }
    } else if let InputParam::Selection(label, options) = input_data.input_param {
        rsx! {
            LabeledSelect {
                id: input_data.id,
                label,
                options,
                readonly: input_data.readonly,
                onchange: move |e| input_data.callback.call(e),
            }
        }
    } else if let InputParam::SIUnit(label, base_unit) = input_data.input_param {
        rsx! {
            NodeConfigUnitInput {
                id: input_data.id,
                label,
                value: input_data.value.parse::<f64>().unwrap_or_default(),
                unit_config: UnitHandling::new(&base_unit, true),
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

/// Renders a list of input components arranged in two-column grid rows. An input with a tooltip
/// shows it when the pointer rests anywhere on its column.
#[component]
pub fn RowedInputs(inputs: Vec<InputData>) -> Element {
    debug!("🔄 Render: RowedInputs");

    rsx! {
        // Standard slice chunks yield sub-slices without any intermediate heap allocation
        for (row_idx , chunk) in inputs.chunks(2).enumerate() {
            div {
                // Key ensures fast and stable Virtual DOM reconciliation
                key: "input_row_{row_idx}",
                class: "row gy-1 gx-2 mb-1",

                match chunk {
                    // Two inputs in a row: Split equally into two columns
                    [first, second] => rsx! {
                        div { class: "col-sm-6", title: first.tooltip.clone().unwrap_or_default(),
                            InputParamLabeledInput { input_data: first.clone() }
                        }
                        div { class: "col-sm-6", title: second.tooltip.clone().unwrap_or_default(),
                            InputParamLabeledInput { input_data: second.clone() }
                        }
                    },
                    // Single trailing input: Render in a half-width column for visual consistency
                    [single] => rsx! {
                        div { class: "col-sm-6", title: single.tooltip.clone().unwrap_or_default(),
                            InputParamLabeledInput { input_data: single.clone() }
                        }
                    },
                    _ => rsx! {},
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
    value: ReadSignal<String>,
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

#[derive(Clone, PartialEq)]
pub struct UnitHandling {
    /// The base unit of the input field. Eg. "m" for length, "W" for power, etc.
    /// This base unit will (optionally) be prefixed with the typical SI prefixes (m, k, M, etc.)
    pub base_unit: String,
    /// Determines, if the input field should hand SI prefixes. If `true`, values like `1000` will be
    /// displayed as `1k`, and `0.001` will be displayed as `1m`. Also, when the user inputs values
    /// with SI prefixes, they will be correctly parsed. If `false`, no SI prefix handling will be applied.
    pub handle_prefix: bool,
    /// Multiplier from internal SI base unit to display base unit (`1.0` for SI units, `180.0 / PI` for `"°"`).
    pub display_scale: f64,
}
impl UnitHandling {
    pub fn new(base_unit: &str, handle_prefix: bool) -> Self {
        Self {
            base_unit: base_unit.to_string(),
            handle_prefix,
            display_scale: 1.0,
        }
    }
    #[must_use]
    pub const fn with_display_scale(mut self, display_scale: f64) -> Self {
        self.display_scale = display_scale;
        self
    }
}
impl From<&ParameterUnit> for UnitHandling {
    fn from(unit: &ParameterUnit) -> Self {
        Self {
            base_unit: unit.base_unit.clone(),
            handle_prefix: unit.handle_prefix,
            display_scale: unit.display_scale,
        }
    }
}
/// Defines whether [`OptionalNumericInput`] operates as a plain scalar or with physical units.
#[derive(Clone, PartialEq, Default)]
pub enum NumericUnitMode {
    /// Dimensionless `f64` supporting `.` and `,` as decimal separators and scientific notation.
    #[default]
    Dimensionless,
    /// Physical quantity in SI base units with optional unit/prefix parsing and unit display feedback.
    Unit(UnitHandling),
}
/// Formats an optional `f64` according to the active [`NumericUnitMode`].
fn format_numeric_opt(opt_val: Option<f64>, mode: &NumericUnitMode, reciprocal: bool) -> String {
    opt_val.map_or_else(String::new, |num| match mode {
        NumericUnitMode::Dimensionless => format_exp_number_notation(num).trim().to_string(),
        NumericUnitMode::Unit(unit_cfg) => format_si_with_base_unit(num, unit_cfg, reciprocal),
    })
}
/// Reusable numeric input supporting `Option<f64>`, `.` and `,` decimal separators,
/// optional SI units (falling back to the base unit if omitted), and commit on blur or `Enter`.
#[component]
pub fn OptionalNumericInput(
    value: ReadSignal<Option<f64>>,
    on_commit: EventHandler<Option<f64>>,
    #[props(default = NumericUnitMode::Dimensionless)] mode: NumericUnitMode,
    #[props(default = false)] allow_empty: bool,
    #[props(default = false)] disabled: bool,
    #[props(default = false)] reciprocal: bool,
    #[props(default = String::new())] class: String,
    #[props(default = String::new())] style: String,
    #[props(default = String::new())] placeholder: String,
) -> Element {
    let val_memo = use_memo(use_reactive!(|value| *value.read()));
    let mode_memo = use_memo(use_reactive!(|mode| mode));
    let mut text = use_signal(|| format_numeric_opt(*value.read(), &mode_memo.peek(), reciprocal));

    use_effect(move || {
        let formatted = format_numeric_opt(*val_memo.read(), &mode_memo.read(), reciprocal);
        if formatted != *text.peek() {
            text.set(formatted);
        }
    });

    // All captured variables (`Signal`, `Memo`, `EventHandler`, `bool`) are `Copy`,
    // so `commit` itself is `Copy` and can be used across multiple event handlers.
    let mut commit = move || {
        if disabled {
            return;
        }
        let raw = text.read().trim().to_string();
        if raw.is_empty() && allow_empty {
            text.set(String::new());
            on_commit.call(None);
            return;
        }

        let current_mode = mode_memo.peek().clone();
        let parsed_opt = match &current_mode {
            NumericUnitMode::Dimensionless => parse_exp_input_strict(&raw)
                .ok()
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|v| v.is_finite()),
            NumericUnitMode::Unit(unit_cfg) => {
                parse_unit_input_permissive(&raw, &unit_cfg.base_unit)
                    .ok()
                    .and_then(|(num_str, prefix_str)| {
                        parse_si_number(&num_str, &prefix_str, reciprocal)
                    })
                    .map(|display_val| display_val / unit_cfg.display_scale)
                    .filter(|v| v.is_finite())
            }
        };

        if let Some(parsed) = parsed_opt {
            text.set(format_numeric_opt(Some(parsed), &current_mode, reciprocal));
            on_commit.call(Some(parsed));
        } else {
            // Revert invalid text to the last confirmed value
            text.set(format_numeric_opt(
                *val_memo.peek(),
                &current_mode,
                reciprocal,
            ));
        }
    };

    let effective_class = if class.is_empty() {
        "form-control form-control-sm bg-black text-light border-secondary p-0 px-1 font-monospace text-end"
            .to_string()
    } else {
        class
    };

    let effective_style = if style.is_empty() {
        "height: 22px; font-size: 11px;".to_string()
    } else {
        style
    };

    rsx! {
        input {
            r#type: "text",
            class: "{effective_class}",
            style: "{effective_style}",
            placeholder: "{placeholder}",
            disabled,
            value: "{text}",
            oninput: move |e: FormEvent| text.set(e.value()),
            onblur: move |_| commit(),
            onkeydown: move |e: KeyboardEvent| {
                if e.key() == Key::Enter {
                    commit();
                }
                e.stop_propagation();
            },
        }
    }
}
#[component]
pub fn NodeConfigUnitInput(
    id: String,
    label: String,
    value: ReadSignal<f64>,
    unit_config: UnitHandling,
    onchange: EventHandler<f64>,
    #[props(default = false)] reciprocal: bool,
    #[props(default = false)] readonly: bool,
) -> Element {
    rsx! {
        UnitInput {
            id,
            label,
            value,
            unit_config,
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
            on_save: on_input_submission,
        }
    }
}

/// A labeled integer input for node configuration panels, backed by [`FlushableTextInput`].
///
/// Accepts non-negative integers (`usize`). Invalid input (non-integer, negative, or a value the
/// core refuses) is logged and the field reverts to the last accepted value — the caller never
/// receives a value the core would reject.
///
/// # Props
///
/// * `id` - the input's own id, which its label points at.
/// * `label` - what the field is called.
/// * `value` - the current value as the document holds it. Re-synced whenever it changes.
/// * `onchange` - called with the parsed integer when the user commits a valid edit.
#[component]
pub fn NodeConfigUsizeInput(
    id: String,
    label: String,
    value: usize,
    onchange: EventHandler<usize>,
) -> Element {
    let mut last_value = use_signal(|| value);
    let mut val_str = use_signal(|| format!("{value}"));

    if *last_value.peek() != value {
        last_value.set(value);
        val_str.set(format!("{value}"));
    }

    let on_input_submission = EventHandler::new(move |val: String| {
        if let Ok(parsed) = val.trim().parse::<usize>() {
            onchange.call(parsed);
        } else {
            OPOSSUM_UI_LOGS
                .write()
                .add_log(&format!("'{val}' is not a valid positive integer"));
        }
        // Pessimistic revert: show the last confirmed value while waiting for the parent to push
        // the accepted value back through the `value` prop. The sync guard above will then
        // overwrite this with the new value if the core accepted it.
        val_str.set(format!("{}", *last_value.peek()));
    });

    rsx! {
        FlushableTextInput {
            id,
            label,
            value: val_str,
            r#type: "number",
            step: Some("1"),
            container_class: "form-floating border-start".to_string(),
            input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
            label_class: "form-label text-secondary".to_string(),
            on_save: on_input_submission,
        }
    }
}

#[component]
pub fn UnitInput(
    id: String,
    label: String,
    value: ReadSignal<f64>,
    unit_config: UnitHandling,
    onchange: EventHandler<f64>,
    #[props(default = false)] reciprocal: bool,
    #[props(default = String::new())] container_class: String,
    #[props(default = String::new())] mut input_class: String,
    #[props(default = String::new())] label_class: String,
    #[props(default = false)] readonly: bool,
    #[props(default = false)] flushable_input: bool,
    #[props(optional)] reset_trigger: Option<ReadSignal<usize>>,
) -> Element {
    let mut val_str =
        use_signal(|| format_si_with_base_unit(*value.read(), &unit_config, reciprocal));

    let val_memo = use_memo(use_reactive!(|value| *value.read()));

    use_effect({
        let current_unit_config = unit_config.clone();
        move || {
            let new_str =
                format_si_with_base_unit(*val_memo.read(), &current_unit_config, reciprocal);
            if new_str != *val_str.peek() {
                val_str.set(new_str);
            }
        }
    });

    // Revert displayed string to the last confirmed prop value when requested by parent
    use_effect({
        let current_unit_config = unit_config.clone();
        move || {
            if let Some(trigger) = reset_trigger {
                let _ = trigger.read();
                let reverted_str =
                    format_si_with_base_unit(*val_memo.read(), &current_unit_config, reciprocal);
                val_str.set(reverted_str);
            }
        }
    });

    let on_input_submission = EventHandler::new({
        let label = label.clone();
        move |val: String| {
            let old_val = val_str();
            if let Ok((num_str, prefix_str)) = parse_unit_input_strict(&val, &unit_config.base_unit)
            {
                if let Some(parsed) = parse_si_number(&num_str, &prefix_str, reciprocal) {
                    val_str.set(format_si_with_base_unit(parsed, &unit_config, reciprocal));
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
                        .add_log(&format!("Wrong input format for field `{label}`! Must have unit `{}` and a valid prefix.",unit_config.base_unit));
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
                on_save: on_input_submission,
            }
        }
    } else {
        if readonly {
            input_class = format!("{input_class} ref-connection-input");
        }
        let reset_gen = reset_trigger.map_or(0, |t| *t.read());
        rsx! {
            input {
                key: "{id}-{reset_gen}",
                class: input_class,
                id,
                value: val_str,
                readonly,
                onchange: move |e: Event<FormData>| on_input_submission.call(e.data.value()),
                onkeydown: move |e: Event<KeyboardData>| {
                    e.stop_propagation();
                },
            }
        }
    }
}

/// A stylized floating-label select input component.
#[component]
pub fn LabeledSelect(
    /// Unique HTML ID for the select element.
    id: String,

    /// Label text displayed in the floating header.
    label: String,

    /// Options list where each entry consists of `(is_selected, value_and_display_text)`.
    options: Vec<(bool, String)>,

    /// Event handler emitting the selected string value directly.
    onchange: EventHandler<Event<FormData>>,

    /// If true, disables user interaction.
    #[props(default = false)]
    readonly: bool,
) -> Element {
    debug!("🔄 Render: LabeledSelect");

    // `bg-dark` is set explicitly so the select keeps a dark field even outside an
    // `.accordion-body` (e.g. the analyzer sidebar, whose sub-editors are loose children of
    // `#accordionAnalyzerConfig`). Inside an accordion the more specific `.accordion-body ...
    // .form-select` rule in `mdb_accordion.css` still wins, so node-config dropdowns are
    // unchanged; without this the select would fall back to MDB's white `.form-select` default.
    let select_class = if readonly {
        "form-select bg-dark text-light disabled-select"
    } else {
        "form-select bg-dark text-light"
    };

    rsx! {
        div { class: "form-floating border-start", "data-mdb-input-init": "",
            select {
                class: select_class,
                id: "{id}",
                disabled: readonly,
                "aria-label": "{label}",
                // Extract the value directly from the DOM event and emit the clean String
                onchange: move |e: Event<FormData>| {
                    onchange.call(e);
                },
                // Use key for fast VDOM list reconciliation and avoid .clone()
                for (is_selected , option) in &options {
                    option {
                        key: "{option}",
                        selected: *is_selected,
                        value: "{option}",
                        "{option}"
                    }
                }
            }
            label { class: "text-secondary", r#for: "{id}", "{label}" }
        }
    }
}
