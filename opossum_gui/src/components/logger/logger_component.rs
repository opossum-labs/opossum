use crate::OPOSSUM_UI_LOGS;
use dioxus::prelude::*;

#[component]
pub fn Logger(drag_handler: EventHandler<f64>, height: ReadSignal<f64>) -> Element {
    rsx! {
        div { class: "row footer",
            div {
                class: "height_resizer",
                onmousedown: move |e: MouseEvent| {
                    drag_handler.call(e.client_coordinates().y);
                },
            }
            div { class: "col",
                div {
                    class: "log-container noselect",
                    style: "height: {height}px; overflow-y: auto;z-index: 99999;",
                    h5 { "Logs" }
                    for log in (OPOSSUM_UI_LOGS.read().logs)().iter().rev() {
                        div { class: "small", "{log}" }
                    }
                }
            }
        }
    }
}
