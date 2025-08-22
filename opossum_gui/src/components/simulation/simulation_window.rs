#![allow(clippy::derive_partial_eq_without_eq)]
use std::process::Stdio;

use dioxus::prelude::*;
use futures_util::StreamExt;
use tokio::{io::{AsyncReadExt, BufReader}, process::Child};
// Define a message to control the coroutine
enum CommandAction {
    Run,
    Abort,
}

#[component]
pub fn SimulationWindow(mut show_simulation: Signal<bool>) -> Element {
    let mut output = use_signal(String::new);
    let mut is_running = use_signal(|| false);
    // Add this block to your component

    // use_effect(move || {
    //     if show_simulation() {
    //         logs.clear();
    //         // Spawn a new asynchronous task
    //         spawn(async move {
    //             // Establish the connection to the SSE endpoint
    //             // Create a reqwest client
    //             let client = reqwest::Client::new();

    //             // Build and send a POST request to the new endpoint
    //             let response = match client
    //                 .post("http://127.0.0.1:8001/api/scenery/simulate")
    //                 .send()
    //                 .await
    //             {
    //                 Ok(res) => res,
    //                 Err(err) => {
    //                     logs.write()
    //                         .push((Level::Error, format!("Connection error: {err}")));
    //                     return;
    //                 }
    //             };
    //             simulation_running.set(true);
    //             // Get the response body as a stream of bytes
    //             let mut stream = response.bytes_stream();

    //             // Process the stream
    //             while let Some(item) = stream.next().await {
    //                 match item {
    //                     Ok(bytes) => {
    //                         // Convert the bytes to a string
    //                         let chunk = String::from_utf8_lossy(&bytes);

    //                         // SSE messages are separated by double newlines.
    //                         // A single chunk from the stream can contain multiple messages.
    //                         for line in chunk.split("\n\n") {
    //                             // SSE data lines start with "data: "
    //                             if let Some(data) = line.strip_prefix("data: ") {
    //                                 if !data.trim().is_empty() {
    //                                     // Split in log_level and message
    //                                     let log_message: Vec<&str> = data.split("##").collect();
    //                                     let log_level = Level::from_str(log_message[0].trim())
    //                                         .unwrap_or(Level::Error);
    //                                     // Push the new log message into our signal
    //                                     logs.write()
    //                                         .push((log_level, log_message[1].trim().to_string()));
    //                                 }
    //                             }
    //                         }
    //                     }
    //                     Err(err) => {
    //                         logs.write()
    //                             .push((Level::Error, format!("Stream error: {err}")));
    //                         break;
    //                     }
    //                 }
    //             }
    //             simulation_running.set(false);
    //         });
    //     }
    // });
    let command_runner = use_coroutine(
        move |mut rx: UnboundedReceiver<CommandAction>| async move {
            #[allow(unused_assignments)] // avoid false positive...
            let mut child_handle: Option<Child> = None;

            while let Some(action) = rx.next().await {
                match action {
                    CommandAction::Run => {
                        is_running.set(true);
                        output.set(String::new());

                        let mut cmd = tokio::process::Command::new(
                            "C:/Users/ueisenb/AppData/Local/0_gsi_executables/opossum/target/debug/opossum.exe",
                        );
                        cmd.arg("-r").arg("C:/Users/ueisenb/AppData/Local/0_gsi_executables/opossum/opossum/playground");
                        cmd.arg("-f").arg("C:/Users/ueisenb/AppData/Local/0_gsi_executables/opossum/opossum/playground/ray_propagation.opm");

                        #[cfg(windows)]
                        {
                            const CREATE_NO_WINDOW: u32 = 0x08000000;
                            cmd.creation_flags(CREATE_NO_WINDOW);
                        }
                        cmd.stdout(Stdio::piped());
                        cmd.stderr(Stdio::piped());

                        let mut child = match cmd.spawn() {
                            Ok(child) => child,
                            Err(e) => {
                                output.set(format!("[ERROR] Failed to spawn command: {}", e));
                                is_running.set(false);
                                continue;
                            }
                        };

                        let stdout = child
                            .stdout
                            .take()
                            .expect("child did not have a handle to stdout");
                        let stderr = child
                            .stderr
                            .take()
                            .expect("child did not have a handle to stderr");
                        let mut stdout_reader = BufReader::new(stdout);
                        let mut stderr_reader = BufReader::new(stderr);
                        child_handle = Some(child);

                        let mut stdout_buf = [0; 1024];
                        let mut stderr_buf = [0; 1024];

                        loop {
                            tokio::select! {
                                // This branch handles aborting the process
                                maybe_action = rx.next() => {
                                    if let Some(CommandAction::Abort) = maybe_action {
                                        if let Some(mut child) = child_handle.take() {
                                            if let Err(e) = child.kill().await {
                                                output.write().push_str(&format!("\n[ERROR] Failed to abort process: {}", e));
                                            } else {
                                                output.write().push_str("\n[INFO] Process aborted by user.");
                                            }
                                        }
                                        break;
                                    }
                                }
                                // Read raw bytes from stdout
                                result = stdout_reader.read(&mut stdout_buf) => {
                                    match result {
                                        Ok(0) => break, // EOF, stream closed.
                                        Ok(n) => {
                                            let s = String::from_utf8_lossy(&stdout_buf[..n]);
                                            output.write().push_str(&s);
                                        },
                                        Err(_) => break,
                                    }
                                },
                                // Read raw bytes from stderr
                                result = stderr_reader.read(&mut stderr_buf) => {
                                     match result {
                                        Ok(0) => {}, // EOF, but stdout might still be writing.
                                        Ok(n) => {
                                            let s = String::from_utf8_lossy(&stderr_buf[..n]);
                                            output.write().push_str(&s);
                                        },
                                        Err(_) => {} // Error, but stdout might still be writing.
                                    }
                                }
                            }
                        }
                        // Ensure the process is fully waited on after the loop exits
                        if let Some(mut child) = child_handle.take() {
                            let _ = child.wait().await;
                        }
                        is_running.set(false);
                    }
                    CommandAction::Abort => {}
                }
            }
        },
    );
    use_effect(move || {
        if show_simulation() {
            command_runner.send(CommandAction::Run);
        } else {
            // NEW: If the window is closed while running, abort the process.
            if is_running() {
                command_runner.send(CommandAction::Abort);
            }
        }
    });
    if show_simulation() {
        rsx! {
            div { class: "modal d-block", "tabindex": "-1",
                div { class: "modal-dialog modal-dialog-centered modal-xl",
                    div { class: "modal-content bg-dark text-white",
                        div { class: "modal-header",
                            h5 { class: "modal-title", "Running simulation..." }
                            button {
                                class: "btn-close btn-close-white",
                                disabled: is_running(),
                                "data-bs-dismiss": "modal",
                                onclick: move |_| show_simulation.set(false),
                            }
                        }
                        div {
                            class: "modal-body",
                            style: "height: 200px; overflow: auto; font-size: 12px;",
                            pre { code { "{output}" } }
                        }
                        div { class: "modal-footer",
                            button {
                                class: "btn btn-secondary",
                                "data-bs-dismiss": "modal",
                                onclick: move |_| {
                                    if is_running() {
                                        command_runner.send(CommandAction::Abort);
                                    } else {
                                        show_simulation.set(false)
                                    }
                                },
                                if is_running() {
                                    "Abort"
                                } else {
                                    "Close"
                                }
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
