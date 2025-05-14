use crate::OPOSSUM_UI_LOGS;
use dioxus::prelude::*;

#[component]
pub fn Logger() -> Element {
    rsx! {
        div { class: "log-container",
            h5 { "Logs" }
            for log in OPOSSUM_UI_LOGS.read().logs.iter() {
                div { class: "small", "{log}" }
            }
        }
    }
}
