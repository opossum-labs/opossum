use crate::components::primitives::{
    alert_dialog::{
        AlertDialog, AlertDialogActions, AlertDialogCancel, AlertDialogDescription,
        AlertDialogTitle,
    },
    button::{Button, ButtonSize, ButtonVariant},
    scroll_area::ScrollArea,
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaBullseye, FaPlus, FaTrash, FaWaveSquare},
};
use dioxus_primitives::alert_dialog::AlertDialogContent;
use opossum_optimization::{DiscoveredObservable, StandardOperand, TargetValueOperand};

use super::state::OptimizationModel;

#[derive(Props, Clone, PartialEq)]
pub struct MeritDesignerProps {
    pub model: Signal<OptimizationModel>,
}

#[component]
pub fn MeritDesignerPanel(mut props: MeritDesignerProps) -> Element {
    let mut show_add_dialog = use_signal(|| false);
    let operands = props.model.read().operands.clone();

    rsx! {
      div { class: "border rounded p-2 bg-dark text-light h-100 d-flex flex-column",
        div { class: "d-flex justify-content-between align-items-center mb-2 pb-2 border-bottom border-secondary gap-2",
          div { class: "fw-bold small d-flex align-items-center gap-1",
            Icon { icon: FaBullseye }
            "Merit Operands ({operands.len()})"
          }
          Button {
            variant: ButtonVariant::Primary,
            size: ButtonSize::Sm,
            title: "Add optimization target or constraint",
            onclick: move |_| show_add_dialog.set(true),
            Icon { icon: FaPlus }
            "Operand"
          }
        }

        ScrollArea { height: "31rem",
          div { class: "table-responsive",
            table {
              class: "table table-dark table-hover table-sm align-middle mb-0 font-monospace",
              style: "font-size: 11px;",
              thead { class: "table-active sticky-top",
                tr {
                  th { "Channel / Target" }
                  th { style: "width: 75px;", "Target" }
                  th { style: "width: 65px;", "Weight" }
                  th { style: "width: 25px;", "" }
                }
              }
              tbody {
                for (idx , op) in operands.iter().enumerate() {
                  {
                      let (badge_text, detail_text) = props.model.read().describe_operand(op);
                      let (target_val, weight_val) = match op {
                          StandardOperand::Target(t) => (t.target, t.weight),
                          StandardOperand::Threshold(t) => (t.limit, t.weight),
                          StandardOperand::MinEdgeThickness(t) => (t.min_thickness, t.weight),
                          StandardOperand::MaxTotalLength(t) => (t.max_length, t.weight),
                          StandardOperand::TransmissionPenalty(t) => (0.0, t.weight),
                      };

                      rsx! {
                        tr { key: "op_{idx}",
                          // Operand & Associated Analyzer
                          td {
                            div { class: "d-flex align-items-center gap-1 mb-1",
                              span {
                                class: "badge bg-primary text-light font-sans text-truncate",
                                style: "max-width: 140px;",
                                "{badge_text}"
                              }
                            }
                            div {
                              class: "fw-bold text-info text-truncate",
                              style: "max-width: 170px;",
                              title: "{detail_text}",
                              "{detail_text}"
                            }
                          }

                          // Target Value Input
                          td { class: "px-1",
                            input {
                              r#type: "number",
                              step: "any",
                              class: "form-control form-control-sm bg-black text-light border-secondary p-0 px-1 font-monospace text-end",
                              style: "height: 22px; font-size: 11px;",
                              value: "{target_val:.4}",
                              oninput: move |e| {
                                  if let Ok(val) = e.value().parse::<f64>() {
                                      props.model.write().update_operand_target(idx, val);
                                  }
                              },
                            }
                          }

                          // Weight Input
                          td { class: "px-1",
                            input {
                              r#type: "number",
                              step: "0.1",
                              class: "form-control form-control-sm bg-black text-light border-secondary p-0 px-1 font-monospace text-end",
                              style: "height: 22px; font-size: 11px;",
                              value: "{weight_val:.1}",
                              oninput: move |e| {
                                  if let Ok(val) = e.value().parse::<f64>() {
                                      props.model.write().update_operand_weight(idx, val);
                                  }
                              },
                            }
                          }

                          // Delete
                          td { class: "text-end px-1",
                            button {
                              class: "btn btn-outline-danger btn-sm p-0 border-0",
                              style: "line-height: 1;",
                              title: "Remove operand",
                              onclick: move |_| props.model.write().remove_operand(idx),
                              Icon { icon: FaTrash }
                            }
                          }
                        }
                      }
                  }
                }

                if operands.is_empty() {
                  tr {
                    td {
                      colspan: "4",
                      class: "text-center text-muted py-4",
                      "No operands specified. Click '+ Operand' to define optimization goals."
                    }
                  }
                }
              }
            }
          }
        }
      }

      if show_add_dialog() {
        AddOperandDialog { model: props.model, open: show_add_dialog }
      }
    }
}

/// Dialog allowing user to select observable metrics grouped by analyzer and detector node.
#[component]
fn AddOperandDialog(mut model: Signal<OptimizationModel>, mut open: Signal<bool>) -> Element {
    let schema_opt = model.read().schema.clone();

    // Grouping: analyzer_name -> (analyzer_kind, Vec<(node_name, Vec<DiscoveredObservable>)>)
    let mut grouped_hierarchy: Vec<(String, String, Vec<(String, Vec<DiscoveredObservable>)>)> =
        Vec::new();

    if let Some(schema) = &schema_opt {
        for obs in &schema.observable_metrics {
            // 1. Analyzer Level
            let channel_entry = if let Some(pos) = grouped_hierarchy
                .iter()
                .position(|(a, _, _)| a == &obs.analyzer_name)
            {
                &mut grouped_hierarchy[pos].2
            } else {
                grouped_hierarchy.push((
                    obs.analyzer_name.clone(),
                    obs.analyzer_kind.clone(),
                    Vec::new(),
                ));
                &mut grouped_hierarchy.last_mut().unwrap().2
            };

            // 2. Detector Node Level
            if let Some(node_pos) = channel_entry.iter().position(|(n, _)| n == &obs.node_name) {
                channel_entry[node_pos].1.push(obs.clone());
            } else {
                channel_entry.push((obs.node_name.clone(), vec![obs.clone()]));
            }
        }
    }

    rsx! {
      AlertDialog {
        open: open(),
        on_open_change: move |v| open.set(v),
        max_width: "46rem".to_string(),
        AlertDialogContent {
          AlertDialogTitle { "Add Merit Function Operand" }
          AlertDialogDescription {
            div { class: "d-flex flex-column gap-3 text-light",
              p { class: "small text-muted mb-0",
                "Choose an observable metric from an active analyzer and detector node:"
              }

              if !grouped_hierarchy.is_empty() {
                ScrollArea { height: "22rem",
                  div { class: "d-flex flex-column gap-2",
                    for (analyzer_name , analyzer_kind , detector_nodes) in grouped_hierarchy {
                      div { class: "border border-secondary rounded overflow-hidden bg-black bg-opacity-50",
                        // Level 1: Analyzer Header
                        div { class: "bg-black py-1 px-2 text-info fw-bold small d-flex align-items-center justify-content-between border-bottom border-secondary",
                          div { class: "d-flex align-items-center gap-2",
                            Icon { icon: FaWaveSquare }
                            span { "Channel: {analyzer_name}" }
                          }
                          span {
                            class: "badge bg-secondary font-monospace",
                            style: "font-size: 9px;",
                            "{analyzer_kind}"
                          }
                        }

                        // Level 2: Detector Nodes
                        div { class: "p-2 d-flex flex-column gap-2",
                          for (node_name , metrics) in detector_nodes {
                            div { class: "rounded border border-secondary border-opacity-50 bg-dark p-2",
                              div { class: "fw-bold text-light small mb-1 pb-1 border-bottom border-secondary border-opacity-25 d-flex align-items-center justify-content-between",
                                span { "{node_name}" }
                                span {
                                  class: "badge bg-secondary font-monospace",
                                  style: "font-size: 9px;",
                                  "{metrics.len()} observables"
                                }
                              }

                              div { class: "list-group list-group-flush",
                                for obs in metrics {
                                  button {
                                    r#type: "button",
                                    class: "list-group-item list-group-item-action bg-transparent text-light py-1 px-2 d-flex justify-content-between align-items-center rounded border-0",
                                    onclick: {
                                        let obs_clone = obs.clone();
                                        move |_| {
                                            let op = StandardOperand::Target(TargetValueOperand {
                                                name: format!("{}: {}", obs_clone.node_name, obs_clone.descriptor.name),
                                                source: obs_clone.address.clone().into(),
                                                target: 0.0,
                                                weight: 1.0,
                                                scale: 1.0e-6,
                                            });
                                            model.write().add_operand(op);
                                            open.set(false);
                                        }
                                    },
                                    div { class: "font-monospace text-light",
                                      "{obs.descriptor.name}"
                                    }
                                    span {
                                      class: "badge bg-primary",
                                      style: "font-size: 9px;",
                                      "+ Select"
                                    }
                                  }
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              } else {
                div { class: "text-muted small",
                  "No observable metrics found in the current analyzers."
                }
              }
            }
          }
          AlertDialogActions {
            AlertDialogCancel { on_click: move |_| open.set(false), "Cancel" }
          }
        }
      }
    }
}
