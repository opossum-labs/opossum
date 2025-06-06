use dioxus::{html::input::checked, prelude::*};

use crate::components::node_editor::node_editor_component::NodeChange;

#[component]
pub fn AccordionItem(elements: Vec<Element>, header: &'static str, header_id: &'static str, parent_id: &'static str, content_id: &'static str) -> Element{
    rsx!{
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
                            { element }
                        }
                    }
                }
            }

    }
}

pub fn name_onchange(
    mut signal: Signal<Option<NodeChange>>,
) -> Callback<Event<FormData>> {
    use_callback(move |e: Event<FormData>| {
        let Ok(name) = e.data.value().parse::<String>();
        signal.set(Some(NodeChange::Name(name)));
    })
}

#[component]
pub fn LabeledInput(
    id: &'static str,
    label: &'static str,
    value: String,
    onchange: Callback<Event<FormData>>,

    #[props(default = "text")]
    r#type: &'static str,

    #[props(optional)]
    step: Option<&'static str>,

    #[props(optional)]
    min: Option<&'static str>,

    #[props(optional)]
    max: Option<&'static str>,
    
    #[props(default = false)]
    hidden: bool,

    #[props(default = false)]
    readonly: bool,
) -> Element {
    rsx! {
        div {
            class: "form-floating border-start",
            "data-mdb-input-init": "",
            hidden: hidden,

            input {
                class: "form-control bg-dark text-light form-control-sm",
                id: id,
                name: id,
                placeholder: label,
                value: value,
                readonly: readonly,
                r#type: r#type,
                step: step.unwrap_or_default(),
                min: min.unwrap_or_default(),
                max: max.unwrap_or_default(),
                onchange: move |e| onchange(e),
            }

            label {
                class: "form-label text-secondary",
                r#for: id,
                "{label}"
            }
        }
    }
}