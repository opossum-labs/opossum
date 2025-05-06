use crate::{api, HTTP_API_CLIENT, OPOSSUM_UI_LOGS};
use dioxus::prelude::*;
use opossum_backend::AnalyzerType;

#[component]
pub fn AnalyzersMenu(analyzer_selected: Signal<AnalyzerType>) -> Element {
    let future = use_resource({
        move || async move {
            match api::get_analyzer_types(&HTTP_API_CLIENT()).await {
                Ok(analyzer_types) => Some(analyzer_types),
                Err(err_str) => {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                    None
                }
            }
        }
    });

    let analyzer_list = match &*future.read_unchecked() {
        Some(Some(response)) => response
            .iter()
            .map(|n| (n.to_owned(), format!("{n}")))
            .collect::<Vec<(AnalyzerType, String)>>(),
        Some(None) => vec![],
        _ => vec![],
    };
    rsx! {
        for element in analyzer_list.into_iter() {
            {
                rsx! {
                    li {
                        a {
                            class: "dropdown-item",
                            role: "button",
                            onclick: move |_| analyzer_selected.set(element.clone().0),
                            {element.clone().1}
                        }
                    }
                }
            }
        }
    }
}
