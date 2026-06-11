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
// Message types to control the simulation execution
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
    let mut simulation_success = use_signal(|| false); // Track if the simulation finished successfully

    let mut close_and_refocus = move || {
        // 1. Close window (change state)
        show_simulation.set(false);

        // 2. Asynchronously refocus the app-container via JavaScript
        spawn(async move {
            let _ = document::eval(
                "let container = document.querySelector('.app-container');
                if (container) {container.focus();}",
            )
            .await;
        });
    };

    let command_runner = use_coroutine(
        move |mut rx: UnboundedReceiver<CommandAction>| async move {
            #[allow(unused_assignments)]
            let mut child_handle: Option<Child> = None;

            while let Some(action) = rx.next().await {
                match action {
                    CommandAction::Run => {
                        is_running.set(true);
                        simulation_success.set(false); // Reset success state for a new run
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

                        // Corrected function name to reflect the actual project API
                        eval_action_run(
                            api::get_document().await,
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
                                let _ = writeln!(
                                    output.write(),
                                    "[ERROR] Failed to spawn command: {e}"
                                );
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
                                let _ = writeln!(output.write(), "[ERROR] Failed to kill process: {e}");
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

                        // Check the exit status of the process to determine actual success
                        let exit_success = if let Some(mut child) = child_handle.take()
                            && let Ok(status) = child.wait().await
                        {
                            status.success()
                        } else {
                            false
                        };

                        if exit_success {
                            output
                                .write()
                                .push_str("\n[INFO] Simulation finished successfully.\n");
                            simulation_success.set(true);
                        } else {
                            output.write().push_str(
                                "\n[ERROR] Simulation terminated with an error or was aborted.\n",
                            );
                            simulation_success.set(false);
                        }
                        is_running.set(false);
                    }
                    CommandAction::Abort => {
                        is_running.set(false);
                    }
                }
            }
        },
    );

    // Fixed: Clean effect tracking solely show_simulation to prevent accidental re-runs
    use_effect(move || {
        if show_simulation() {
            command_runner.send(CommandAction::Run);
        } else {
            command_runner.send(CommandAction::Abort);
        }
    });

    if !show_simulation() {
        return rsx! {};
    }

    // Closure to handle opening the reports, defined before RSX to keep UI code clean
    let open_reports = move |_| {
        if let Some(dir) = project_directory() {
            let mut i = 0;
            let mut opened_any = false;

            loop {
                let report_filename = format!("report_{i}.html");
                let report_path = dir.join(&report_filename);

                if report_path.exists() {
                    if let Some(path_str) = report_path.to_str() {
                        match webbrowser::open(path_str) {
                            Ok(()) => {
                                let _ = writeln!(output.write(), "[INFO] Opened {report_filename}");
                                opened_any = true;
                            }
                            Err(e) => {
                                let _ = writeln!(
                                    output.write(),
                                    "[ERROR] Failed to open {report_filename}: {e}"
                                );
                            }
                        }
                    }
                    i += 1;
                } else {
                    break;
                }
            }

            if !opened_any {
                output
                    .write()
                    .push_str("[WARN] No reports found to open in the project directory.\n");
            }
        } else {
            output
                .write()
                .push_str("[ERROR] Cannot open reports: No project directory set.\n");
        }
    };

    rsx! {
        div {
            class: "modal d-block",
            "tabindex": "-1",
            style: "background-color: rgba(0,0,0,0.5);",
            onkeydown: move |evt| {
                if evt.key() == Key::Escape && !is_running() {
                    close_and_refocus();
                }
            },
            onmounted: async move |evt| {
                let _ = evt.set_focus(true).await;
            },
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
                            disabled: is_running(),
                            onclick: move |_| {
                                if is_running() {
                                    command_runner.send(CommandAction::Abort);
                                } else {
                                    close_and_refocus();
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
                        // Dynamic status formatting
                        if is_running() {
                            span { class: "me-auto text-info small", "Simulation is running..." }
                        } else if simulation_success() {
                            span { class: "me-auto text-success small", "Process finished successfully." }
                        } else {
                            span { class: "me-auto text-danger small", "Process failed or aborted." }
                        }

                        // Open Reports Button (only rendered when simulation succeeded)
                        if simulation_success() {
                            button {
                                class: "btn btn-primary",
                                title: "Open generated reports in your default web browser",
                                onclick: open_reports, // Clean assignment
                                "Open Reports"
                            }
                        }

                        button {
                            class: if is_running() { "btn btn-danger" } else { "btn btn-secondary" },
                            title: if is_running() { "Abort the running simulation" } else { "Close window (Esc)" },
                            onclick: move |_| {
                                if is_running() {
                                    command_runner.send(CommandAction::Abort);
                                } else {
                                    close_and_refocus();
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
