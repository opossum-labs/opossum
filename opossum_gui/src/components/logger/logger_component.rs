use crate::OPOSSUM_UI_LOGS;
use dioxus::prelude::*;

#[component]
pub fn Logger() -> Element {
    let log_id = "log-container".to_owned();
    rsx! {
        div { class: "log-container", id: log_id,
            h5 { "Logs" }
            for log in OPOSSUM_UI_LOGS.read().logs.iter() {
                p {
                    class: "log-entry",
                    onmounted: {
                        let log_id = log_id.clone();
                        move |_| {
                            let script = format!(
                                r#"document.getElementById("{log_id}").scrollTop = document.getElementById("{log_id}").scrollHeight;"#,
                            );
                            dioxus::document::eval(&script);
                        }
                    },
                    "{log}"
                }
            }
        }
    }
}
