use crate::api::{
    eval_action_run, get_optimization_schema, post_abort_optimization,
    post_save_optimization_config, post_start_optimization, subscribe_optimization_events,
    undo_document,
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
        FaBullseye, FaCube, FaPlay, FaRotateLeft, FaStop, FaTriangleExclamation,
        FaWandMagicSparkles,
    },
};
use opossum_optimization::{ModelOptimizationSchema, OptimizationProgressEvent};

use super::{
    live_monitor_panel::LiveMonitorPanel,
    merit_designer_panel::{AddOperandDialog, MeritDesignerPanel},
    state::{OptimizationModel, ValidationSeverity},
    variables_panel::{AddVariableDialog, CouplingDialog, VariablesPanel},
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
    let mut show_assistants = use_signal(|| false);
    let mut steps_without_improvement = use_signal(|| 0_u64);
    let mut best_cost = use_signal(|| f64::INFINITY);

    // Dialog state signals managed at the root level to prevent table stacking context clipping
    let show_add_var_dialog = use_signal(|| false);
    let show_coupling_dialog = use_signal(|| false);
    let show_add_operand_dialog = use_signal(|| false);

    // Initial schema retrieval when the modal dialog is opened
    use_effect(move || {
        if open() {
            spawn(async move {
                let res = get_optimization_schema().await;
                eval_action_run(
                    res,
                    Some(move |schema: ModelOptimizationSchema| {
                        model.write().populate_from_schema(schema);
                    }),
                );
            });
        }
    });

    // Start optimization job and dispatch SSE stream into the coroutine channel
    let mut start_optimization = move || {
        let config = match model.read().build_optimization_config() {
            Ok(cfg) => cfg,
            Err(err) => {
                log::error!("Cannot start optimization: {err}");
                return;
            }
        };

        max_iterations.set(model.read().max_iterations());
        steps_without_improvement.set(0);
        best_cost.set(f64::INFINITY);
        is_running.set(true);

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<OptimizationProgressEvent>();

        // 1. UI-Thread Task: Updates Dioxus state upon receiving events
        spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    OptimizationProgressEvent::Started {
                        initial_cost: cost,
                        total_max_iterations,
                    } => {
                        initial_cost.set(cost);
                        current_cost.set(cost);
                        best_cost.set(cost);
                        steps_without_improvement.set(0);
                        max_iterations.set(total_max_iterations);
                    }
                    OptimizationProgressEvent::Iteration {
                        iteration,
                        best_cost: new_best,
                        ..
                    } => {
                        current_iteration.set(iteration);
                        current_cost.set(new_best);

                        let prev = *best_cost.read();
                        // Relative improvement threshold with lower floor for small costs
                        let min_improvement = prev.abs().max(1e-12) * 1e-6;
                        if (prev - new_best) > min_improvement {
                            best_cost.set(new_best);
                            steps_without_improvement.set(0);
                        } else {
                            *steps_without_improvement.write() += 1;
                        }
                    }
                    OptimizationProgressEvent::Finished {
                        final_cost,
                        optimized_parameters,
                        ..
                    } => {
                        current_cost.set(final_cost);
                        is_running.set(false);
                        model.write().update_values(&optimized_parameters);
                        *crate::NODE_DETAILS_REFRESH.write() += 1;
                    }
                    OptimizationProgressEvent::Aborted {
                        last_cost,
                        best_parameters,
                        ..
                    } => {
                        if !last_cost.is_nan() {
                            current_cost.set(last_cost);
                        }
                        is_running.set(false);
                        if !best_parameters.is_empty() {
                            model.write().update_values(&best_parameters);
                        }
                        *crate::NODE_DETAILS_REFRESH.write() += 1;
                    }
                    OptimizationProgressEvent::Failed { error } => {
                        log::error!("Optimization failed: {error}");
                        is_running.set(false);
                    }
                }
            }
        });

        // 2. Background Task: Ensures SSE receiver is connected BEFORE sending /start
        spawn(async move {
            let sse_tx = tx.clone();
            let sse_fail_tx = tx.clone();
            let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<()>();

            spawn(async move {
                if let Err(err) = subscribe_optimization_events(Some(ready_tx), move |event| {
                    let _ = sse_tx.send(event);
                })
                .await
                {
                    log::error!("SSE stream error: {err}");
                    let _ = sse_fail_tx.send(OptimizationProgressEvent::Failed {
                        error: format!("SSE stream failed: {err}"),
                    });
                }
            });

            // Await deterministic connection readiness instead of sleeping
            if ready_rx.await.is_err() {
                log::error!("Failed to establish SSE connection before start.");
                is_running.set(false);
                return;
            }

            let res = post_start_optimization(config).await;
            if let Err(err) = res {
                log::error!("Failed to start optimization: {err}");
                is_running.set(false);
            }
        });
    };

    // Immediate abort reaction
    let mut abort_optimization = move || {
        is_running.set(false);
        spawn(async move {
            let res = post_abort_optimization().await;
            eval_action_run(res, Some(|msg| log::info!("{msg}")));
        });
    };

    // Valid start condition: Enabled variables > 0, Operands > 0, No fatal configuration errors
    let can_start = model.read().can_start_optimization();

    // Revert to initial baseline via backend undo stack
    let revert_to_baseline = move || {
        spawn(async move {
            let res = undo_document().await;
            eval_action_run(
                res,
                Some(|_| {
                    *crate::NODE_DETAILS_REFRESH.write() += 1;
                }),
            );

            let schema_res = get_optimization_schema().await;
            eval_action_run(
                schema_res,
                Some(move |schema| {
                    let mut m = model.write();
                    m.populate_from_schema(schema);
                    m.reset_to_initial_baseline();
                }),
            );
        });
    };

    let sync_config_to_backend = move || {
        let config = model.read().to_optimization_config();
        spawn(async move {
            let _ = post_save_optimization_config(config).await;
        });
    };

    rsx! {
      AlertDialog {
        open: open(),
        on_open_change: move |v: bool| {
          if !v {
            sync_config_to_backend();
          }
          open.set(v);
        },
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
                      value: model
                          .read()
                          .scope_group_id
                          .map(|id| id.to_string())
                          .unwrap_or_else(|| "all".to_string()),
                      onchange: move |e| {
                          let val = e.value();
                          if val == "all" {
                              model.write().set_scope(None);
                          } else if let Ok(parsed_id) = uuid::Uuid::parse_str(&val) {
                              model.write().set_scope(Some(parsed_id));
                          }
                      },
                      option { value: "all", "Entire Optical Setup" }
                      for (group_id , group_name) in model.read().available_groups() {
                        option { value: "{group_id}", "{group_name}" }
                      }
                    }
                  }

                  // Rule Assistants Menu (Controlled directly via Dioxus state)
                  div { class: "dropdown position-relative",
                    button {
                      class: "btn btn-outline-info btn-sm dropdown-toggle d-flex align-items-center gap-1",
                      style: "height: 30px; font-size: 11px;",
                      r#type: "button",
                      onclick: move |e: MouseEvent| {
                          e.stop_propagation();
                          show_assistants.set(!show_assistants());
                      },
                      Icon { icon: FaWandMagicSparkles }
                      "Rule Assistants"
                    }

                    if show_assistants() {
                      div {
                        style: "position: fixed; inset: 0; z-index: 999;",
                        onclick: move |_| show_assistants.set(false),
                      }

                      ul {
                        class: "dropdown-menu dropdown-menu-dark shadow small show position-absolute",
                        style: "z-index: 1000; top: 100%; left: 0; display: block; min-width: 14rem;",
                        li {
                          a {
                            class: "dropdown-item",
                            href: "#",
                            onclick: move |e| {
                                e.prevent_default();
                                model.write().apply_default_bounds(0.2);
                                show_assistants.set(false);
                            },
                            "Add Default Box Bounds (±20%)"
                          }
                        }
                        li {
                          a {
                            class: "dropdown-item",
                            href: "#",
                            onclick: move |e| {
                                e.prevent_default();
                                model.write().apply_default_bounds(0.5);
                                show_assistants.set(false);
                            },
                            "Add Wide Box Bounds (±50%)"
                          }
                        }
                      }
                    }
                  }

                  // Run / Abort Control
                  Button {
                    variant: if is_running() { ButtonVariant::Destructive } else { ButtonVariant::Primary },
                    size: ButtonSize::Sm,
                    disabled: !is_running() && !can_start,
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

              // 3-Column Layout with Root Signals
              div { class: "row g-3",
                div { class: "col-lg-5 col-md-12",
                  VariablesPanel {
                    model,
                    show_add_dialog: show_add_var_dialog,
                    show_coupling_dialog,
                  }
                }
                div { class: "col-lg-4 col-md-12",
                  MeritDesignerPanel {
                    model,
                    show_add_dialog: show_add_operand_dialog,
                  }
                }
                div { class: "col-lg-3 col-md-12",
                  LiveMonitorPanel {
                    model,
                    is_running: is_running(),
                    iteration: current_iteration(),
                    current_cost: current_cost(),
                    initial_cost: initial_cost(),
                    steps_without_improvement: steps_without_improvement(),
                  }
                }
              }
            }
          }
        }
        AlertDialogActions {
          AlertDialogCancel {
            on_click: move |_| {
              sync_config_to_backend();
              open.set(false);
            },
            "Close Window"
          }
        }
      }

      // Root-level sub-dialogs (rendered outside the table-scroll and card stacking contexts)
      if show_add_var_dialog() {
        AddVariableDialog { model, open: show_add_var_dialog }
      }

      if show_coupling_dialog() {
        CouplingDialog { model, open: show_coupling_dialog }
      }

      if show_add_operand_dialog() {
        AddOperandDialog { model, open: show_add_operand_dialog }
      }
    }
}
