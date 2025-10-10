#![allow(clippy::derive_partial_eq_without_eq)]
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use opossum_backend::AnalyzerType;

#[component]
pub fn AnalyzersMenu(on_analyzer_selected: EventHandler<AnalyzerType>) -> Element {
    let future = use_resource({
        move || async move {
            match api::get_analyzer_types().await {
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
        _ => vec![],
    };
    rsx! {
        for (analyzer_type , analyzer_name) in analyzer_list.into_iter() {
            {
                rsx! {
                    li {
                        a {
                            class: "dropdown-item",
                            role: "button",
                            onclick: move |_| on_analyzer_selected.call(analyzer_type.clone()),
                            {analyzer_name}
                        }
                    }
                }
            }
        }
    }
}
