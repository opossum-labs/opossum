#![allow(clippy::derive_partial_eq_without_eq)]
use dioxus::prelude::*;

#[component]
pub fn AccordionItem(
    elements: Vec<Element>,
    header: String,
    header_id: String,
    parent_id: String,
    content_id: String,
    level: usize,
) -> Element {
    rsx! {
        div { class: "accordion-item bg-dark text-light",
            h6 { class: "accordion-header", id: &header_id,
                button {
                    class: "accordion-button accordion-button-h{level} collapsed bg-dark text-light",
                    r#type: "button",
                    "data-mdb-collapse-init": "",
                    "data-mdb-target": format!("#{content_id}"),
                    "aria-expanded": "false",
                    "aria-controls": &content_id,
                    {header}
                }
            }
            div {
                id: &content_id,
                class: "accordion-collapse collapse  bg-dark",
                "aria-labelledby": &header_id,
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
pub fn ElementList(element_list: Vec<Element>) -> Element {
    rsx! {
        div { class: "bg-dark",
            for element in element_list {
                {element}
            }
        }
    }
}
