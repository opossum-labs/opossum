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

// Nachrichtentypen zur Steuerung der Simulation
enum CommandAction {
    Run,
    Abort,
}

#[component]
pub fn SimulationWindow(
    show_simulation: Signal<bool>,
    project_directory: ReadSignal<Option<PathBuf>>,
) -> Element {
    let mut output = use_signal(String::new);
    let mut is_running = use_signal(|| false);
    let command_runner = use_coroutine(
        move |mut rx: UnboundedReceiver<CommandAction>| async move {
            #[allow(unused_assignments)]
            let mut child_handle: Option<Child> = None;

            while let Some(action) = rx.next().await {
                match action {
                    CommandAction::Run => {
                        is_running.set(true);
                        output.set(String::new());
                        output.write().push_str("[INFO] Preparing simulation...\n");
                        let Ok(temp_dir) = tempdir() else {
                            output
                                .write()
                                .push_str("[ERROR] Could not create temp dir.\n");
                            is_running.set(false);
                            continue;
                        };
                        let temp_model_file = temp_dir.path().join("temp-opossum.opm");
                        let temp_model_file_clone = temp_model_file.clone();
                        eval_action_run(
                            api::get_opm_file().await,
                            Some(move |opm_string| {
                                if let Err(err_str) = fs::write(temp_model_file, opm_string) {
                                    OPOSSUM_UI_LOGS.write().add_log(&err_str.to_string());
                                }
                            }),
                        );
                        let Ok(cli_path) = find_cli_executable() else {
                            output
                                .write()
                                .push_str("[ERROR] Opossum CLI executable not found.\n");
                            is_running.set(false);
                            continue;
                        };
                        let Some(report_dir) = project_directory() else {
                            output
                                .write()
                                .push_str("[ERROR] No project directory set.\n");
                            is_running.set(false);
                            continue;
                        };
                        let mut cmd = tokio::process::Command::new(cli_path);
                        cmd.arg("-r")
                            .arg(report_dir.as_path())
                            .arg("-f")
                            .arg(temp_model_file_clone)
                            .arg("-s")
                            .arg("false") // Silent mode (no Logo)
                            .stdout(Stdio::piped())
                            .stderr(Stdio::piped());

                        #[cfg(windows)]
                        {
                            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                            cmd.creation_flags(CREATE_NO_WINDOW);
                        }
                        output.write().push_str("[INFO] Starting CLI process...\n");
                        let mut child = match cmd.spawn() {
                            Ok(child) => child,
                            Err(e) => {
                                writeln!(output.write(), "[ERROR] Failed to spawn command: {e}")
                                    .unwrap();
                                is_running.set(false);
                                continue;
                            }
                        };
                        let stdout = child.stdout.take().expect("Failed to open stdout");
                        let stderr = child.stderr.take().expect("Failed to open stderr");
                        let mut stdout_reader = BufReader::new(stdout);
                        let mut stderr_reader = BufReader::new(stderr);

                        child_handle = Some(child);
                        let mut stdout_buf = [0u8; 1024];
                        let mut stderr_buf = [0u8; 1024];
                        let mut stdout_closed = false;
                        let mut stderr_closed = false;

                        loop {
                            tokio::select! {
                                maybe_action = rx.next() => {
                                    if matches!(maybe_action, Some(CommandAction::Abort)) {
                                        if let Some(mut child) = child_handle.take() {
                                            output.write().push_str("\n[WARN] Aborting process requested by user...\n");
                                            if let Err(e) = child.kill().await {
                                                writeln!(output.write(), "[ERROR] Failed to kill process: {e}").unwrap();
                                            } else {
                                                output.write().push_str("[INFO] Process aborted successfully.\n");
                                            }
                                        }
                                        break;
                                    }
                                }
                                result = stdout_reader.read(&mut stdout_buf), if !stdout_closed => {
                                    match result {
                                        Ok(0) | Err(_) => stdout_closed = true,
                                        Ok(n) => {
                                            let s = String::from_utf8_lossy(&stdout_buf[..n]);
                                            output.write().push_str(&s);
                                        },
                                    }
                                },
                                result = stderr_reader.read(&mut stderr_buf), if !stderr_closed => {
                                    match result {
                                        Ok(0) | Err(_) => stderr_closed = true,
                                        Ok(n) => {
                                            let s = String::from_utf8_lossy(&stderr_buf[..n]);
                                            output.write().push_str(&s);
                                        },
                                    }
                                }
                            }

                            if stdout_closed && stderr_closed {
                                break;
                            }
                        }
                        if let Some(mut child) = child_handle.take() {
                            let _ = child.wait().await;
                        }

                        output.write().push_str("\n[INFO] Simulation finished.\n");
                        is_running.set(false);
                    }
                    CommandAction::Abort => {
                        show_simulation.set(false);
                    }
                }
            }
        },
    );

    use_effect(move || {
        if show_simulation() {
            command_runner.send(CommandAction::Run);
        } else if is_running() {
            command_runner.send(CommandAction::Abort);
        }
    });
    if !show_simulation() {
        return rsx! {};
    }
    rsx! {
        div {
            class: "modal d-block",
            "tabindex": "-1",
            style: "background-color: rgba(0,0,0,0.5);",
            div { class: "modal-dialog modal-dialog-centered modal-xl",
                div { class: "modal-content bg-dark text-white",
                    div { class: "modal-header",
                        h5 { class: "modal-title d-flex align-items-center gap-2",
                            "Simulation Output"
                            if is_running() {
                                span {
                                    class: "spinner-border spinner-border-sm text-info",
                                    role: "status",
                                    span { class: "visually-hidden", "Loading..." }
                                }
                            }
                        }
                        button {
                            class: "btn-close btn-close-white",
                            disabled: is_running(), // avoids closing the window while still running
                            onclick: move |_| {
                                if is_running() {
                                    command_runner.send(CommandAction::Abort);
                                } else {
                                    show_simulation.set(false);
                                }
                            },
                        }
                    }
                    div { class: "modal-body",
                        div { style: "overflow-y: auto; max-height: 400px; background-color: #1e1e1e; padding: 10px; border-radius: 4px;",
                            pre { style: "margin: 0; font-size: 11px; font-family: 'Consolas', monospace; white-space: pre-wrap;",
                                code { "{output}" }
                            }
                            div { id: "log-end-anchor" }
                        }
                    }
                    div { class: "modal-footer",
                        // Status Text
                        if is_running() {
                            span { class: "me-auto text-info small", "Simulation is running..." }
                        } else {
                            span { class: "me-auto text-success small", "Process finished." }
                        }
                        button {
                            class: if is_running() { "btn btn-danger" } else { "btn btn-secondary" },
                            onclick: move |_| {
                                if is_running() {
                                    command_runner.send(CommandAction::Abort);
                                } else {
                                    show_simulation.set(false);
                                }
                            },
                            if is_running() {
                                "Abort Simulation"
                            } else {
                                "Close"
                            }
                        }
                    }
                }
            }
        }
    }
}
