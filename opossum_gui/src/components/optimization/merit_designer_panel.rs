use crate::components::primitives::{
    alert_dialog::{
        AlertDialog, AlertDialogActions, AlertDialogCancel,
        AlertDialogDescription, AlertDialogTitle,
    },
    button::{Button, ButtonSize, ButtonVariant},
    scroll_area::ScrollArea,
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaCrosshairs, FaPlus, FaTrash},
};
use dioxus_primitives::alert_dialog::AlertDialogContent;
use opossum_core::core_optics::observable::MetricDimension;
use opossum_optimization::{
   MetricSource, MinEdgeThicknessOperand, Operand, StandardOperand, TargetValueOperand, ThresholdDirection, ThresholdOperand, operand::default_scale_for_metric,
};

use super::state::OptimizationModel;

#[derive(Props, Clone, PartialEq)]
pub struct MeritDesignerProps {
    pub model: Signal<OptimizationModel>,
}

struct OperandDisplayInfo {
    category: &'static str,
    target_node: String,
    target_numeric: f64,
}

fn resolve_operand_display(op: &StandardOperand) -> OperandDisplayInfo {
    match op {
        StandardOperand::Target(t) => {
            let cat = match &t.source {
                MetricSource::Observable(obs) => {
                    let metric = obs.metric_name.to_lowercase();
                    if metric.contains("spot") || metric.contains("radius") {
                        "Focus"
                    } else if metric.contains("wavefront") || metric.contains("p-v") {
                        "Wavefront"
                    } else {
                        "Detector"
                    }
                }
                MetricSource::Parameter(_) => "Geometry",
            };
            OperandDisplayInfo {
                category: cat,
                target_node: match &t.source {
                    MetricSource::Observable(obs) => obs.metric_name.clone(),
                    MetricSource::Parameter(p) => format!("{p:?}"),
                },
                target_numeric: t.target,
            }
        }
        StandardOperand::Threshold(t) => OperandDisplayInfo {
            category: "Threshold",
            target_node: match &t.source {
                MetricSource::Observable(obs) => obs.metric_name.clone(),
                MetricSource::Parameter(p) => format!("{p:?}"),
            },
            target_numeric: t.limit,
        },
        StandardOperand::MaxTotalLength(t) => OperandDisplayInfo {
            category: "Envelope",
            target_node: format!("{} segments", t.segments.len()),
            target_numeric: t.max_length,
        },
        StandardOperand::MinEdgeThickness(t) => OperandDisplayInfo {
            category: "Geometry",
            target_node: format!("Lens {}", &t.node_id.to_string()[..8]),
            target_numeric: t.min_thickness,
        },
        StandardOperand::TransmissionPenalty(t) => OperandDisplayInfo {
            category: "Loss",
            target_node: t.address.metric_name.clone(),
            target_numeric: t.expected_rays,
        },
    }
}

#[component]
pub fn MeritDesignerPanel(mut props: MeritDesignerProps) -> Element {
    let mut selected_category = use_signal(|| "All");
    let mut show_add_modal = use_signal(|| false);

    let operands = props.model.read().operands.clone();
    let cat_filter = *selected_category.read();
    let total_weight: f64 = operands.iter().map(|op| op.weight()).sum();

    let operand_rows: Vec<(usize, StandardOperand, OperandDisplayInfo, f64)> = operands
        .into_iter()
        .enumerate()
        .map(|(idx, op)| {
            let info = resolve_operand_display(&op);
            let share = if total_weight > 0.0 {
                (op.weight() / total_weight) * 100.0
            } else {
                0.0
            };
            (idx, op, info, share)
        })
        .filter(|(_, _, info, _)| cat_filter == "All" || info.category == cat_filter)
        .collect();

    rsx! {
      div { class: "border rounded p-2 bg-dark text-light h-100 d-flex flex-column",
        div { class: "d-flex justify-content-between align-items-center mb-2 pb-2 border-bottom border-secondary gap-2",
          div { class: "fw-bold small d-flex align-items-center gap-1",
            Icon { icon: FaCrosshairs }
            "Merit Operands ({props.model.read().operands.len()})"
          }
          div { class: "d-flex align-items-center gap-2",
            select {
              class: "form-select form-select-sm bg-black text-light border-secondary py-0 px-2",
              style: "width: 7.5rem; height: 28px; font-size: 11px;",
              value: "{selected_category}",
              onchange: move |e| {
                  selected_category
                      .set(
                          match e.value().as_str() {
                              "Focus" => "Focus",
                              "Wavefront" => "Wavefront",
                              "Geometry" => "Geometry",
                              "Envelope" => "Envelope",
                              _ => "All",
                          },
                      )
              },
              option { value: "All", "All Classes" }
              option { value: "Focus", "Focus Spots" }
              option { value: "Wavefront", "Wavefronts" }
              option { value: "Geometry", "Geometry" }
              option { value: "Envelope", "Envelope" }
            }
            Button {
              variant: ButtonVariant::Primary,
              size: ButtonSize::Sm,
              title: "Add optimization target or constraint from schema",
              onclick: move |_| show_add_modal.set(true),
              Icon { icon: FaPlus }
              "Operand"
            }
          }
        }

        ScrollArea { height: "31rem",
          div { class: "table-responsive",
            table {
              class: "table table-dark table-hover table-sm align-middle mb-0 font-monospace",
              style: "font-size: 11px;",
              thead { class: "table-active sticky-top",
                tr {
                  th { style: "width: 25px;", "" }
                  th { "Criterion / Metric" }
                  th { style: "width: 75px;", "Target" }
                  th { style: "width: 48px;", "Weight" }
                  th {
                    style: "width: 80px;",
                    class: "text-center",
                    "Impact"
                  }
                  th { style: "width: 25px;", "" }
                }
              }
              tbody {
                for (idx , op , info , share) in operand_rows {
                  tr { key: "{idx}_{op.name()}",
                    td { class: "text-center px-1 text-muted",
                      "{idx + 1}"
                    }

                    td {
                      div {
                        class: "fw-bold text-light text-truncate",
                        style: "max-width: 140px;",
                        title: "{op.name()}",
                        "{op.name()}"
                      }
                      div {
                        class: "text-muted text-truncate",
                        style: "max-width: 140px;",
                        "{info.target_node}"
                      }
                    }

                    // Editable Target Goal Input
                    td { class: "px-1",
                      input {
                        r#type: "number",
                        class: "form-control form-control-sm bg-black text-success border-secondary p-0 px-1 font-monospace text-end",
                        style: "height: 22px; font-size: 11px;",
                        value: "{info.target_numeric}",
                        oninput: {
                            move |e| {
                                if let Ok(val) = e.value().parse::<f64>() {
                                    props.model.write().update_operand_target(idx, val);
                                }
                            }
                        },
                      }
                    }

                    // Editable Weight Input
                    td { class: "px-1",
                      input {
                        r#type: "number",
                        class: "form-control form-control-sm bg-black text-light border-secondary p-0 px-1 font-monospace text-end",
                        style: "height: 22px; font-size: 11px;",
                        value: "{op.weight()}",
                        oninput: {
                            move |e| {
                                if let Ok(new_w) = e.value().parse::<f64>() {
                                    props.model.write().update_operand_weight(idx, new_w);
                                }
                            }
                        },
                      }
                    }

                    // In-Bar Impact Widget
                    td { class: "px-1",
                      div {
                        class: "progress position-relative bg-secondary bg-opacity-50",
                        style: "height: 18px; min-width: 70px; border-radius: 3px;",
                        div {
                          class: if share > 35.0 { "progress-bar bg-danger" } else if share > 0.0 { "progress-bar bg-primary" } else { "progress-bar bg-transparent" },
                          style: "width: {share}%;",
                        }
                        span {
                          class: "position-absolute w-100 text-center fw-bold text-light font-monospace",
                          style: "top: 0; left: 0; line-height: 18px; font-size: 10px; text-shadow: 0 0 3px black;",
                          "{share:.0}%"
                        }
                      }
                    }

                    // Delete Action
                    td { class: "text-end px-1",
                      button {
                        class: "btn btn-outline-danger btn-sm p-0 border-0",
                        style: "line-height: 1;",
                        title: "Delete operand",
                        onclick: move |_| props.model.write().remove_operand(idx),
                        Icon { icon: FaTrash }
                      }
                    }
                  }
                }
                if props.model.read().operands.is_empty() {
                  tr {
                    td {
                      colspan: "6",
                      class: "text-center text-muted py-4",
                      "No operands configured. Click '+ Operand' to add criteria."
                    }
                  }
                }
              }
            }
          }
        }
      }

      if show_add_modal() {
        AddOperandDialog { model: props.model, open: show_add_modal }
      }
    }
}

#[component]
fn AddOperandDialog(mut model: Signal<OptimizationModel>, mut open: Signal<bool>) -> Element {
    let schema_opt = model.read().schema.clone();

    rsx! {
      AlertDialog {
        open: open(),
        on_open_change: move |v| open.set(v),
        max_width: "44rem".to_string(),
        AlertDialogContent {
          AlertDialogTitle { "Add Optimization Operand" }
          AlertDialogDescription {
            div { class: "d-flex flex-column gap-3 text-light",
              p { class: "small text-muted mb-0",
                "Choose a detector observable or geometric constraint from the loaded model."
              }

              if let Some(schema) = schema_opt {
                // Section 1: Observable Detector Metrics
                div {
                  div { class: "fw-bold small mb-2 text-info", "Detector Observables" }
                  ScrollArea { height: "12rem",
                    div { class: "list-group list-group-flush small",
                      for obs in schema.observable_metrics {
                        button {
                          r#type: "button",
                          class: "list-group-item list-group-item-action bg-dark text-light border-secondary d-flex justify-content-between align-items-center",
                          onclick: {
                              let obs_clone = obs.clone();
                              move |_| {
                                  let scale = default_scale_for_metric(
                                      obs_clone.descriptor.dimension,
                                      &obs_clone.descriptor.name,
                                  );
                                  let operand = TargetValueOperand::new(
                                      format!("{} ({})", obs_clone.descriptor.name, obs_clone.node_name),
                                      obs_clone.address.clone(),
                                      0.0,
                                      1.0,
                                      scale,
                                  );
                                  model.write().add_operand(operand.into());
                                  open.set(false);
                              }
                          },
                          div {
                            div { class: "fw-bold", "{obs.node_name}" }
                            div { class: "text-muted font-monospace",
                              "{obs.descriptor.name} ({obs.descriptor.dimension:?})"
                            }
                          }
                          span { class: "badge bg-primary",
                            "+ Add Target"
                          }
                        }
                      }
                    }
                  }
                }

                // Section 2: Edge Thickness Candidates
                if !schema.edge_thickness_nodes.is_empty() {
                  div {
                    div { class: "fw-bold small mb-2 text-warning",
                      "Physical Lens Constraints"
                    }
                    div { class: "d-flex flex-column gap-1",
                      for edge_node in schema.edge_thickness_nodes {
                        button {
                          r#type: "button",
                          class: "btn btn-outline-secondary btn-sm text-start d-flex justify-content-between align-items-center",
                          onclick: {
                              let node_id = edge_node.node_id;
                              let node_name = edge_node.node_name.clone();
                              move |_| {
                                  let operand = MinEdgeThicknessOperand::new(
                                      format!("Min Edge ({node_name})"),
                                      node_id,
                                      0.0015, // 1.5 mm default
                                      50.0,
                                  );
                                  model.write().add_operand(operand.into());
                                  open.set(false);
                              }
                          },
                          span { "{edge_node.node_name} ({edge_node.node_type})" }
                          span { class: "badge bg-warning text-dark",
                            "Min Edge >= 1.5 mm"
                          }
                        }
                      }
                    }
                  }
                }
              } else {
                div { class: "text-muted small", "No model schema loaded." }
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