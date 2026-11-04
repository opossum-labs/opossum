use crate::api::{
    eval_action_run, get_optimization_schema, post_abort_optimization,
    post_start_optimization, subscribe_optimization_events, undo_document,
};
use crate::components::primitives::{
    alert_dialog::{AlertDialog, AlertDialogActions, AlertDialogCancel, AlertDialogDescription},
    button::{Button, ButtonSize, ButtonVariant},
    card::{Card, CardAction, CardContent, CardHeader, CardTitle},
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{
        FaBullseye, FaCube, FaFileExport, FaPlay, FaRotateLeft, FaStop,
        FaTriangleExclamation, FaWandMagicSparkles,
    },
};
use futures_util::StreamExt;
use opossum_optimization::OptimizationProgressEvent;

use super::{
    live_monitor_panel::LiveMonitorPanel,
    merit_designer_panel::MeritDesignerPanel,
    state::{OptimizationModel, ValidationSeverity},
    variables_panel::VariablesPanel,
};

#[component]
pub fn OptimizationModal(open: Signal<bool>) -> Element {
    let mut model = use_signal(OptimizationModel::default);
    let mut is_running = use_signal(|| false);
    let mut current_iteration = use_signal(|| 0_u64);
    let mut max_iterations = use_signal(|| 100_u64);
    let mut current_cost = use_signal(|| 0.0_f64);
    let mut initial_cost = use_signal(|| 0.0_f64);
    let mut show_issue_drawer = use_signal(|| false);

    // Coroutine receives progress events from background thread and updates UI signals safely
    let sse_receiver = use_coroutine(move |mut rx: UnboundedReceiver<OptimizationProgressEvent>| async move {
        while let Some(event) = rx.next().await {
            match event {
                OptimizationProgressEvent::Started { initial_cost: cost, total_max_iterations } => {
                    initial_cost.set(cost);
                    current_cost.set(cost);
                    max_iterations.set(total_max_iterations);
                }
                OptimizationProgressEvent::Iteration { iteration, best_cost, .. } => {
                    current_iteration.set(iteration);
                    current_cost.set(best_cost);
                }
                OptimizationProgressEvent::Finished { final_cost, .. } => {
                    current_cost.set(final_cost);
                    is_running.set(false);
                    *crate::NODE_DETAILS_REFRESH.write() += 1;
                }
                OptimizationProgressEvent::Aborted { last_cost, .. } => {
                    if !last_cost.is_nan() {
                        current_cost.set(last_cost);
                    }
                    is_running.set(false);
                    *crate::NODE_DETAILS_REFRESH.write() += 1;
                }
                OptimizationProgressEvent::Failed { error } => {
                    log::error!("Optimization failed: {error}");
                    is_running.set(false);
                }
            }
        }
    });

    // Initial schema retrieval when the modal dialog is opened
    use_effect(move || {
        if open() && model.read().schema.is_none() {
            spawn(async move {
                let res = get_optimization_schema().await;
                eval_action_run(res, Some(move |schema| {
                    model.write().populate_from_schema(schema);
                }));
            });
        }
    });

    // Start optimization job and dispatch SSE stream into the coroutine channel
    // Start optimization job and dispatch SSE stream via Tokio mpsc channel
    let mut start_optimization = move || {
        let config = match model.read().build_optimization_config() {
            Ok(cfg) => cfg,
            Err(err) => {
                log::error!("Cannot start optimization: {err}");
                return;
            }
        };

        is_running.set(true);

        // Standard Tokio Channel: Tx is Send + 'static, Rx stays on the UI thread
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<OptimizationProgressEvent>();

        // 1. UI-Thread Task: Receives events from channel and updates local signals
        spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    OptimizationProgressEvent::Started { initial_cost: cost, total_max_iterations } => {
                        initial_cost.set(cost);
                        current_cost.set(cost);
                        max_iterations.set(total_max_iterations);
                    }
                    OptimizationProgressEvent::Iteration { iteration, best_cost, .. } => {
                        current_iteration.set(iteration);
                        current_cost.set(best_cost);
                    }
                    OptimizationProgressEvent::Finished { final_cost, .. } => {
                        current_cost.set(final_cost);
                        is_running.set(false);
                        *crate::NODE_DETAILS_REFRESH.write() += 1;
                    }
                    OptimizationProgressEvent::Aborted { last_cost, .. } => {
                        if !last_cost.is_nan() {
                            current_cost.set(last_cost);
                        }
                        is_running.set(false);
                        *crate::NODE_DETAILS_REFRESH.write() += 1;
                    }
                    OptimizationProgressEvent::Failed { error } => {
                        log::error!("Optimization failed: {error}");
                        is_running.set(false);
                    }
                }
            }
        });

        // 2. Background Task: Streams SSE events and triggers the backend start endpoint
        spawn(async move {
            // Task 2a: SSE Stream listener (tx is Send)
            let sse_tx = tx.clone();
            spawn(async move {
                let _ = subscribe_optimization_events(move |event| {
                    let _ = sse_tx.send(event);
                }).await;
            });

            // Task 2b: Trigger backend optimization worker
            let res = post_start_optimization(config).await;
            if let Err(err) = res {
                log::error!("Failed to start optimization: {err}");
                is_running.set(false);
            }
        });
    };

    // Abort active solver execution
    let abort_optimization = move || {
        spawn(async move {
            let res = post_abort_optimization().await;
            eval_action_run(res, Some(|msg| log::info!("{msg}")));
        });
    };

    // Revert to initial baseline via backend undo stack
    let revert_to_baseline = move || {
        spawn(async move {
            let res = undo_document().await;
            eval_action_run(res, Some(|_| {
                *crate::NODE_DETAILS_REFRESH.write() += 1;
            }));
        });
    };

    let has_fatal_errors = model
        .read()
        .validation_issues
        .iter()
        .any(|i| i.severity == ValidationSeverity::Error);

    rsx! {
      AlertDialog {
        open: open(),
        on_open_change: move |v: bool| open.set(v),
        max_width: "90rem".to_string(),
        AlertDialogDescription {
          Card {
            CardHeader {
              CardTitle {
                span { class: "d-flex align-items-center gap-2 text-white",
                  Icon { icon: FaBullseye }
                  "System Optimization"
                }
              }
              CardAction {
                div { class: "d-flex align-items-center gap-2 flex-wrap",
                  // Subsystem Scope Selector
                  div { class: "d-flex align-items-center gap-1",
                    span { class: "text-muted small",
                      Icon { icon: FaCube }
                    }
                    select {
                      class: "form-select form-select-sm bg-black text-light border-secondary font-monospace py-0 px-2",
                      style: "width: 15.5rem; height: 30px; font-size: 11px;",
                      option { value: "all", selected: true,
                        "Entire Optical Setup"
                      }
                    }
                  }

                  // Rule Assistants Menu
                  div { class: "dropdown",
                    button {
                      class: "btn btn-outline-info btn-sm dropdown-toggle d-flex align-items-center gap-1",
                      style: "height: 30px; font-size: 11px;",
                      r#type: "button",
                      "data-bs-toggle": "dropdown",
                      Icon { icon: FaWandMagicSparkles }
                      "Rule Assistants"
                    }
                    ul { class: "dropdown-menu dropdown-menu-dark shadow small",
                      li {
                        a {
                          class: "dropdown-item",
                          href: "#",
                          onclick: move |_| {
                              for row in &mut model.write().variable_rows {
                                  let val = row.current_value;
                                  row.min_bound = Some(val * 0.8);
                                  row.max_bound = Some(val * 1.2);
                              }
                              model.write().run_preflight_validation();
                          },
                          "Add Default Box Bounds (±20%)"
                        }
                      }
                    }
                  }

                  // Run / Abort Control
                  Button {
                    variant: if is_running() { ButtonVariant::Destructive } else { ButtonVariant::Primary },
                    size: ButtonSize::Sm,
                    disabled: !is_running() && has_fatal_errors,
                    onclick: move |_| {
                        if is_running() {
                            abort_optimization();
                        } else {
                            start_optimization();
                        }
                    },
                    if is_running() {
                      Icon { icon: FaStop }
                      "Abort"
                    } else {
                      Icon { icon: FaPlay }
                      "Run Optimization"
                    }
                  }

                  // Undo baseline restoration
                  Button {
                    variant: ButtonVariant::Outline,
                    size: ButtonSize::Sm,
                    disabled: is_running(),
                    onclick: move |_| revert_to_baseline(),
                    Icon { icon: FaRotateLeft }
                    "Undo to Baseline"
                  }
                }
              }
            }

            CardContent {
              // Pre-Flight Sanity Warning Drawer
              if !model.read().validation_issues.is_empty() {
                div { class: "mb-3 rounded border border-warning bg-warning bg-opacity-10 p-2",
                  div { class: "d-flex justify-content-between align-items-center",
                    div { class: "d-flex align-items-center gap-2 text-warning fw-bold small",
                      Icon { icon: FaTriangleExclamation }
                      "Pre-Flight Sanity Check: {model.read().validation_issues.len()} Issue(s)"
                    }
                    button {
                      class: "btn btn-outline-warning btn-sm py-0 px-2 small",
                      onclick: move |_| show_issue_drawer.set(!show_issue_drawer()),
                      if show_issue_drawer() {
                        "Hide Details"
                      } else {
                        "Review & Fix"
                      }
                    }
                  }
                  if show_issue_drawer() {
                    div { class: "mt-2 pt-2 border-top border-warning border-opacity-25 d-flex flex-column gap-1",
                      for issue in model.read().validation_issues.iter() {
                        div { class: "d-flex justify-content-between align-items-center small py-1 px-2 rounded bg-black bg-opacity-25",
                          span { class: if issue.severity == ValidationSeverity::Error { "text-danger fw-bold" } else { "text-warning" },
                            "{issue.message}"
                          }
                        }
                      }
                    }
                  }
                }
              }

              // 3-Column Layout with Real Data Signals
              div { class: "row g-3",
                div { class: "col-lg-5 col-md-12",
                  VariablesPanel { model }
                }
                div { class: "col-lg-4 col-md-12",
                  MeritDesignerPanel { model }
                }
                div { class: "col-lg-3 col-md-12",
                  LiveMonitorPanel {
                    is_running: is_running(),
                    iteration: current_iteration(),
                    max_iterations: max_iterations(),
                    current_cost: current_cost(),
                    initial_cost: initial_cost(),
                  }
                }
              }
            }
          }
        }
        AlertDialogActions {
          div { class: "d-flex justify-content-between align-items-center w-100",
            Button {
              variant: ButtonVariant::Outline,
              size: ButtonSize::Sm,
              title: "Export configuration profile to JSON",
              Icon { icon: FaFileExport }
              "Export Config"
            }
            AlertDialogCancel { on_click: move |_| open.set(false), "Close Window" }
          }
        }
      }
    }
}