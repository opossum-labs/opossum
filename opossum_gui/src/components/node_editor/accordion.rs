use dioxus::prelude::*;

#[component]
fn Wrapper(children: Element) -> Element {
    rsx! {
        div {
            class: "wrapper",
            h2 { "Ich bin der Wrapper!" },
            {children}
        }
    }
}

#[component]
fn Greeting(name: String) -> Element {
    rsx! {
        p { "Hallo, {name}!" }
    }
}

#[component]
fn App() -> Element {
    rsx! {
        Wrapper {
            Greeting { name: "Anna".to_string() }
        }
    }
}

#[component]
pub fn AccordionItem(elements: Vec<Element>, header: &'static str, id: &'static str, parent: &'static str, content_id: &'static str) -> Element{
    rsx!{
        div { class: "accordion-item bg-dark text-light",
                h6 { class: "accordion-header", id: id,
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
                    "aria-labelledby": id,
                    "data-mdb-parent": format!("#{parent}"),
                    div { class: "accordion-body  bg-dark",
                        for element in elements {
                            { element }
                        }
                        // RayPositionDistributionSelector { light_data_builder_sig }
                        // RayDistributionEditor { light_data_builder_sig }
                    }
                }
            }

    }
}