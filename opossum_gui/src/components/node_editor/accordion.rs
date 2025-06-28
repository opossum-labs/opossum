use dioxus::prelude::*;

#[component]
pub fn AccordionItem(
    elements: Vec<Element>,
    header: &'static str,
    header_id: &'static str,
    parent_id: &'static str,
    content_id: &'static str,
) -> Element {
    rsx! {
        div { class: "accordion-item bg-dark text-light",
            h6 { class: "accordion-header", id: header_id,
                button {
                    class: "accordion-button collapsed bg-dark text-light",
                    r#type: "button",
                    "data-mdb-collapse-init": "",
                    "data-mdb-target": format!("#{content_id}"),
                    "aria-expanded": "false",
                    "aria-controls": content_id,
                    {header}
                }
            }
            div {
                id: content_id,
                class: "accordion-collapse collapse  bg-dark",
                "aria-labelledby": header_id,
                "data-mdb-parent": format!("#{parent_id}"),
                div { class: "accordion-body  bg-dark",
                    for element in elements {
                        {element}
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

    #[props(default = None)] onchange: Option<Callback<Event<FormData>>>,

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
                onchange: onchange.unwrap_or_default(),
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
    #[props(default = false)] hidden: bool,
) -> Element {
    rsx! {
        div {
            hidden,
            class: "form-floating border-start",
            "data-mdb-input-init": "",
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
