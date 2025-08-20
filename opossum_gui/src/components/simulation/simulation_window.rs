use dioxus::prelude::*;
use futures_util::StreamExt;

#[component]
pub fn SimulationWindow(mut show_simulation: Signal<bool>) -> Element {
    let mut logs = use_signal(Vec::<String>::new);
    let mut simulation_running = use_signal(|| false);
    use_effect(move || {
        if show_simulation() {
            logs.clear();
            // Spawn a new asynchronous task
            spawn(async move {
                // Establish the connection to the SSE endpoint
                // Create a reqwest client
                let client = reqwest::Client::new();

                // Build and send a POST request to the new endpoint
                let response = match client
                    .post("http://127.0.0.1:8001/api/scenery/simulate")
                    .send()
                    .await
                {
                    Ok(res) => res,
                    Err(err) => {
                        logs.write().push(format!("Connection error: {}", err));
                        return;
                    }
                };
                simulation_running.set(true);
                // Get the response body as a stream of bytes
                let mut stream = response.bytes_stream();

                // Process the stream
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(bytes) => {
                            // Convert the bytes to a string
                            let chunk = String::from_utf8_lossy(&bytes);

                            // SSE messages are separated by double newlines.
                            // A single chunk from the stream can contain multiple messages.
                            for line in chunk.split("\n\n") {
                                // SSE data lines start with "data: "
                                if let Some(data) = line.strip_prefix("data: ") {
                                    if !data.trim().is_empty() {
                                        // Push the new log message into our signal
                                        logs.write().push(data.trim().to_string());
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            logs.write().push(format!("Stream error: {}", err));
                            break;
                        }
                    }
                }
                simulation_running.set(false);
            });
        }
    });
    if show_simulation() {
        rsx! {
            div {
                class: "modal d-block",
                "tabindex": "-1",
                "data-bs-theme": "light",
                div { class: "modal-dialog modal-dialog-centered",
                    div { class: "modal-content",
                        div { class: "modal-header",
                            h5 { class: "modal-title", "Running simulation..." }
                            button {
                                class: "btn-close",
                                disabled: simulation_running(),
                                "data-bs-dismiss": "modal",
                                onclick: move |_| show_simulation.set(false),
                            }
                        }
                        div {
                            class: "modal-body",
                            style: " height: 200px; overflow: auto; font-size: 12px;",
                            for log_message in logs.read().iter() {
                                "{log_message}"
                                br {}
                            }
                        }
                        div { class: "modal-footer",
                            button {
                                class: "btn btn-secondary",
                                disabled: simulation_running(),
                                "data-bs-dismiss": "modal",
                                onclick: move |_| show_simulation.set(false),
                                "Close"
                            }
                        }
                    }
                }
            }
        }
    } else {
        rsx! {}
    }
}
