use crate::OPOSSUM_UI_LOGS;
use dioxus::{document::eval, prelude::*};
use dioxus_free_icons::{Icon, icons::fa_solid_icons::FaCopy};

#[component]
pub fn Logger(drag_handler: EventHandler<f64>, height: ReadSignal<f64>) -> Element {
    // Safely clone the logs vector to prevent temporary borrowing issues in the loop
    let logs = OPOSSUM_UI_LOGS.read().logs.cloned();

    rsx! {
        div { class: "row footer",
            div {
                class: "resizer height_resizer",
                onmousedown: move |e: MouseEvent| {
                    drag_handler.call(e.client_coordinates().y);
                },
            }
            div { class: "col",
                div {
                    class: "log-container noselect",
                    style: "height: {height}px; overflow-y: auto; z-index: 99999;",
                    h5 { "Logs" }
                    for log in logs.into_iter().rev() {
                        div { class: "d-flex align-items-start justify-content-between mb-1",

                            div { class: "small font-monospace text-break flex-grow-1 mb-0",
                                "{log}"
                            }

                            button {
                                class: "btn btn-sm p-0 border-0 text-secondary ms-2",
                                title: "Copy to clipboard",
                                onclick: move |_| {
                                    let log_to_copy = log.clone();
                                    spawn(async move {
                                        // Minified and cleaned JS payload for cross-environment clipboard support
                                        let js_code = format!(
                                            r#"
                                                                                                                                                        (function() {{
                                                                                                                                                            var text = {log_to_copy:?};
                                                                                                                                                            if (navigator.clipboard) {{
                                                                                                                                                                navigator.clipboard.writeText(text).catch(function() {{ fallback(text); }});
                                                                                                                                                            }} else {{
                                                                                                                                                                fallback(text);
                                                                                                                                                            }}
                                                                                                                                                            function fallback(t) {{
                                                                                                                                                                var ta = document.createElement("textarea");
                                                                                                                                                                ta.value = t;
                                                                                                                                                                ta.style.position = "fixed";
                                                                                                                                                                ta.style.opacity = "0";
                                                                                                                                                                document.body.appendChild(ta);
                                                                                                                                                                ta.select();
                                                                                                                                                                try {{ document.execCommand("copy"); }} catch (e) {{}}
                                                                                                                                                                document.body.removeChild(ta);
                                                                                                                                                            }}
                                                                                                                                                        }})();
                                                                                                                                                        "#,
                                        );

                                        let js = eval(&js_code);
                                        let _ = js.join::<()>().await;
                                    });
                                },
                                Icon {
                                    icon: FaCopy,
                                    width: 14,
                                    height: 14,
                                    fill: "currentColor",
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
