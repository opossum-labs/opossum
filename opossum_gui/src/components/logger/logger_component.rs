use crate::OPOSSUM_UI_LOGS;
use dioxus::prelude::*;

#[component]
pub fn Logger() -> Element {
    rsx! {
        div { class: "log-container",
            h5 { "Logs" }
            for log in OPOSSUM_UI_LOGS.read().logs.iter() {
                div {
                    class: "small",
                    // onmounted: {
                    //     let log_id = log_id.clone();
                    //     move |_| {
                    //         let script = format!(
                    //             r#"document.getElementById("{log_id}").scrollTop = document.getElementById("{log_id}").scrollHeight;"#,
                    //         );
                    //         //dioxus::document::eval(&script);
                    //     }
                    // },
                    "{log}"
                }
            }
        }
    }
}
