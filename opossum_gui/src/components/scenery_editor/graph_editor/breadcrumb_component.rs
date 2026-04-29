use dioxus::prelude::*;
use uuid::Uuid;

#[component]
pub fn BreadCrumbs(
    bread_crumbs: Vec<(Uuid, String)>,
    bread_crumb_click_event: EventHandler<(Uuid, String)>,
) -> Element {
    rsx! {
        div { class: "graph-breadcrumbs",
            for (i , (id , name)) in bread_crumbs.iter().enumerate() {
                {
                    let name = name.clone();
                    let id = *id;
                    rsx! {
                        span {
                            class: "breadcrumb",
                            onclick: move |_| bread_crumb_click_event.call((id, name.clone())),
                            "{name}"
                        }


                        if i < bread_crumbs.len() - 1 {
                            span { class: "breadcrumb-sep", " › " }
                        }
                    }
                }
            }
        }
    }
}
