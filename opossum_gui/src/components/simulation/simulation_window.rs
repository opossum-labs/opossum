#![allow(clippy::derive_partial_eq_without_eq)]
use dioxus::prelude::*;
use futures_util::StreamExt;
use std::{fs, process::Stdio};
use tempfile::tempdir;
use tokio::{
    io::{AsyncReadExt, BufReader},
    process::Child,
};

use crate::{
    OPOSSUM_UI_LOGS,
    api::{self, run_action},
    components::{scenery_editor::NodeEditorCommand, simulation::utils::find_cli_executable},
};

// Define a message to control the coroutine
enum CommandAction {
    Run,
    Abort,
}

#[component]
pub fn SimulationWindow(
    mut show_simulation: Signal<bool>,
    node_editor_command: Signal<Option<NodeEditorCommand>>,
) -> Element {
    let mut output = use_signal(String::new);
    let mut is_running = use_signal(|| false);

    let command_runner = use_coroutine(
        move |mut rx: UnboundedReceiver<CommandAction>| async move {
            #[allow(unused_assignments)] // avoid false positive...
            let mut child_handle: Option<Child> = None;

            while let Some(action) = rx.next().await {
                match action {
                    CommandAction::Run => {
                        is_running.set(true);
                        output.set(String::new());
                        let Ok(temp_dir) = tempdir() else {
                            output.set("Could not determine temp dir.".into());
                            return;
                        };
                        let temp_model_file = temp_dir.path().join("temp-opossum.opm");
                        let temp_model_file_clone = temp_model_file.clone();
                        // We have to this with run_action instead of sending a node_editor_command since we have to be sure
                        // that the file has been written before calling the CLI.
                        run_action(
                            api::get_opm_file(),
                            Some(move |opm_string| {
                                if let Err(err_str) = fs::write(temp_model_file, opm_string) {
                                    OPOSSUM_UI_LOGS.write().add_log(&err_str.to_string());
                                }
                            }),
                        );
                        let Ok(cli_path) = find_cli_executable() else {
                            output.set("Did not find CLI".into());
                            return;
                        };
                        let mut cmd = tokio::process::Command::new(cli_path);
                        cmd.arg("-r").arg("C:/Users/ueisenb/AppData/Local/0_gsi_executables/opossum/opossum/playground");
                        cmd.arg("-f").arg(temp_model_file_clone);
                        cmd.arg("-s").arg("false"); // do not display OPOSSUM logo and version info

                        #[cfg(windows)]
                        {
                            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                            cmd.creation_flags(CREATE_NO_WINDOW);
                        }
                        cmd.stdout(Stdio::piped());
                        cmd.stderr(Stdio::piped());

                        let mut child = match cmd.spawn() {
                            Ok(child) => child,
                            Err(e) => {
                                output.set(format!("[ERROR] Failed to spawn command: {e}"));
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
                                    if matches!(maybe_action, Some(CommandAction::Abort)) {
                                        if let Some(mut child) = child_handle.take() {
                                            if let Err(e) = child.kill().await {
                                                output.write().push_str(&format!("\n[ERROR] Failed to abort process: {e}"));
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
                                        Ok(0) | Err(_) => break, // EOF or Error, stream closed.
                                        Ok(n) => {
                                            let s = String::from_utf8_lossy(&stdout_buf[..n]);
                                            output.write().push_str(&s);
                                        },
                                    }
                                },
                                // Read raw bytes from stderr
                                result = stderr_reader.read(&mut stderr_buf) => {
                                     match result {
                                        Ok(0) | Err(_) => {}, // EOF or Error, but stdout might still be writing.
                                        Ok(n) => {
                                            let s = String::from_utf8_lossy(&stderr_buf[..n]);
                                            output.write().push_str(&s);
                                        },
                                    }
                                }
                            }
                        }
                        // Ensure the process is fully waited on after the loop exits
                        if let Some(mut child) = child_handle.take() {
                            let _ = child.wait().await;
                        }
                        is_running.set(false);
                        temp_dir.close().unwrap();
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
            // If the window is closed while running, abort the process.
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
                        div { class: "modal-body", style: "overflow: auto;",
                            pre { style: "height: 400px; font-size: 10px;",
                                code { "{output}" }
                            }
                        }
                        div { class: "modal-footer",
                            button {
                                class: "btn btn-secondary",
                                "data-bs-dismiss": "modal",
                                onclick: move |_| {
                                    if is_running() {
                                        command_runner.send(CommandAction::Abort);
                                    } else {
                                        show_simulation.set(false);
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
