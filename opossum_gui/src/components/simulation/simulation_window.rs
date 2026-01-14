#![allow(clippy::derive_partial_eq_without_eq)]
use crate::{
    OPOSSUM_UI_LOGS,
    api::{self, eval_action_run},
    components::simulation::utils::find_cli_executable,
};
use dioxus::prelude::*;
use futures_util::StreamExt;
use std::fmt::Write;
use std::{fs, path::PathBuf, process::Stdio};
use tempfile::tempdir;
use tokio::{
    io::{AsyncReadExt, BufReader},
    process::Child,
};

// Define a message to control the coroutine
enum CommandAction {
    Run,
    Abort,
}

#[component]
pub fn SimulationWindow(
    show_simulation: Signal<bool>,
    project_directory: Signal<Option<PathBuf>>,
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
                            is_running.set(false);
                            continue;
                        };
                        let temp_model_file = temp_dir.path().join("temp-opossum.opm");
                        let temp_model_file_clone = temp_model_file.clone();
                        // We have to this with run_action instead of sending a node_editor_command since we have to be sure
                        // that the file has been written before calling the CLI.
                        eval_action_run(
                            api::get_opm_file().await,
                            Some(move |opm_string| {
                                if let Err(err_str) = fs::write(temp_model_file, opm_string) {
                                    OPOSSUM_UI_LOGS.write().add_log(&err_str.to_string());
                                }
                            }),
                        );
                        let Ok(cli_path) = find_cli_executable() else {
                            output.set("Did not find CLI".into());
                            is_running.set(false);
                            continue;
                        };
                        let Some(report_dir) = project_directory() else {
                            output.set("No report directory set".into());
                            is_running.set(false);
                            continue;
                        };
                        let mut cmd = tokio::process::Command::new(cli_path);
                        cmd.arg("-r").arg(report_dir.as_path());
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
                        let mut stdout_closed = false;
                        let mut stderr_closed = false;

                        loop {
                            tokio::select! {
                                // This branch handles aborting the process
                                    maybe_action = rx.next() => {
                                        if matches!(maybe_action, Some(CommandAction::Abort)) {
                                            if let Some(mut child) = child_handle.take() {
                                                if let Err(e) = child.kill().await {
                                                    write!(*output.write(), "\n[ERROR] Failed to abort process: {e}").unwrap();
                                                } else {
                                                    output.write().push_str("\n[INFO] Process aborted by user.");
                                                }
                                            }
                                            break;
                                        }
                                }
                                // Read raw bytes from stdout, but only if the stream isn't closed yet
                                result = stdout_reader.read(&mut stdout_buf), if !stdout_closed => {
                                    match result {
                                        Ok(0) | Err(_) => stdout_closed = true, // Mark as closed, don't break
                                        Ok(n) => {
                                            let s = String::from_utf8_lossy(&stdout_buf[..n]);
                                            output.write().push_str(&s);
                                        },
                                    }
                                },
                                // Read raw bytes from stderr, but only if the stream isn't closed yet
                                result = stderr_reader.read(&mut stderr_buf), if !stderr_closed => {
                                        match result {
                                        Ok(0) | Err(_) => stderr_closed = true, // Mark as closed
                                        Ok(n) => {
                                            let s = String::from_utf8_lossy(&stderr_buf[..n]);
                                            output.write().push_str(&s);
                                        },
                                    }
                                }
                            }
                            // Exit the loop only when both streams are confirmed to be closed
                            if stdout_closed && stderr_closed {
                                break;
                            }
                        }
                        // Ensure the process is fully waited on after the loop exits
                        if let Some(mut child) = child_handle.take() {
                            let _ = child.wait().await;
                        }
                        is_running.set(false);
                        temp_dir.close().unwrap();
                    }
                    CommandAction::Abort => {
                        //todo: really abort the simulation
                        show_simulation.set(false);
                    }
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
                                onclick: move |_| {
                                    if is_running() {
                                        command_runner.send(CommandAction::Abort);
                                    } else {
                                        show_simulation.set(false);
                                    }
                                },
                            }
                        }
                        div { class: "modal-body", style: "overflow: auto;",
                            pre { style: "height: 400px; font-size: 11px;",
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
