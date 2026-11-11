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
    icons::fa_solid_icons::{
        FaBullseye, FaChartLine, FaFilter, FaLayerGroup, FaPlus, FaRulerCombined, FaTrash,
        FaWaveSquare,
    },
};
use dioxus_primitives::alert_dialog::AlertDialogContent;
use opossum_optimization::{
    DiscoveredObservable, MaxTotalLengthOperand, MinEdgeThicknessOperand, ParameterAddress,
    StandardOperand, TargetValueOperand, ThresholdDirection, ThresholdOperand,
    TransmissionPenaltyOperand, default_scale_for_metric,
};

use super::state::OptimizationModel;
use uuid::Uuid;

type DiscoveredObservableHierarchy =
    Vec<(String, String, Vec<(String, Vec<DiscoveredObservable>)>)>;

#[derive(Props, Clone, PartialEq)]
pub struct MeritDesignerProps {
    pub model: Signal<OptimizationModel>,
    pub show_add_dialog: Signal<bool>,
}

#[component]
pub fn MeritDesignerPanel(mut props: MeritDesignerProps) -> Element {
    let operands = props.model.read().operands.clone();
    let operands_count = operands.len();

    rsx! {
      div { class: "border rounded p-2 bg-dark text-light h-100 d-flex flex-column",
        div { class: "d-flex justify-content-between align-items-center mb-2 pb-2 border-bottom border-secondary gap-2",
          div { class: "fw-bold small d-flex align-items-center gap-1",
            Icon { icon: FaBullseye }
            "Merit Operands ({operands_count})"
          }
          Button {
            variant: ButtonVariant::Primary,
            size: ButtonSize::Sm,
            title: "Add optimization target or constraint",
            onclick: move |_| props.show_add_dialog.set(true),
            Icon { icon: FaPlus }
            "Operand"
          }
        }

        ScrollArea { height: "31rem",
          div { class: "table-responsive",
            table {
              class: "table table-dark table-hover table-sm align-middle mb-0 font-monospace",
              style: "font-size: 11px;",
              thead {
                class: "table-active sticky-top",
                style: "z-index: 1;",
                tr {
                  th { style: "width: 25px;", "" } // Checkbox Spalte
                  th { "Channel / Metric" }
                  th {
                    style: "width: 45px;",
                    class: "text-center",
                    "Type"
                  }
                  th { style: "width: 85px;", "Target/Limit" }
                  th { style: "width: 65px;", "Weight" }
                  th { style: "width: 25px;", "" }
                }
              }
              tbody {
                for op_row in operands.iter() {
                  OperandRow {
                    key: "{op_row.id}",
                    operand_id: op_row.id,
                    model: props.model,
                  }
                }

                if operands_count == 0 {
                  tr {
                    td {
                      colspan: "5",
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
    }
}

/// Local controlled numeric input that commits on blur or Enter.
#[component]
fn OperandNumberInput(value: f64, on_commit: EventHandler<f64>) -> Element {
    let format_val = |v: f64| {
        let s = format!("{v:.4}");
        if s.contains('.') {
            let trimmed = s.trim_end_matches('0').trim_end_matches('.');
            if trimmed.is_empty() || trimmed == "-" {
                "0".to_string()
            } else {
                trimmed.to_string()
            }
        } else {
            s
        }
    };

    let val_memo = use_memo(use_reactive!(|value| value));
    let mut text = use_signal(|| format_val(value));

    // Synchronize cleanly when value updates from outside
    use_effect(move || {
        let current = *val_memo.read();
        let formatted = format_val(current);
        if formatted != *text.peek() {
            text.set(formatted);
        }
    });

    let mut commit = move || {
        let raw = text.read().trim().to_string();
        if let Ok(parsed) = raw.parse::<f64>() {
            text.set(format_val(parsed));
            on_commit.call(parsed);
        } else {
            text.set(format_val(*val_memo.peek()));
        }
    };

    rsx! {
      input {
        r#type: "text",
        class: "form-control form-control-sm bg-black text-light border-secondary p-0 px-1 font-monospace text-end",
        style: "height: 22px; font-size: 11px;",
        value: "{text}",
        oninput: move |e: FormEvent| text.set(e.value()),
        onblur: move |_| commit(),
        onkeydown: move |e: KeyboardEvent| {
            if e.key() == Key::Enter {
                commit();
            }
        },
      }
    }
}

#[derive(Props, Clone, PartialEq)]
struct OperandRowProps {
    operand_id: Uuid,
    model: Signal<OptimizationModel>,
}

#[component]
fn OperandRow(mut props: OperandRowProps) -> Element {
    let model_read = props.model.read();
    let op_id = props.operand_id;
    let Some(op_row) = model_read.operands.iter().find(|o| o.id == op_id) else {
        return rsx! {};
    };
    let op = &op_row.operand;
    let is_enabled = op_row.is_enabled;
    let (badge_text, detail_text, op_symbol) = model_read.describe_operand_full(op);
    let (target_val, weight_val) = match op {
        StandardOperand::Target(t) => (t.target, t.weight),
        StandardOperand::Threshold(t) => (t.limit, t.weight),
        StandardOperand::MinEdgeThickness(t) => (t.min_thickness * 1000.0, t.weight),
        StandardOperand::MaxTotalLength(t) => (t.max_length * 1000.0, t.weight),
        StandardOperand::TransmissionPenalty(t) => (t.expected_rays, t.weight),
    };
    let is_threshold = matches!(op, StandardOperand::Threshold(_));
    let is_target = matches!(op, StandardOperand::Target(_));
    let badge_class = if badge_text == "Geometry" {
        "badge bg-warning text-dark font-sans"
    } else {
        "badge bg-primary text-light font-sans text-truncate"
    };
    drop(model_read);

    rsx! {
      tr { class: if !is_enabled { "opacity-50" } else { "" },
        // Checkbox column
        td { class: "text-center px-1",
          input {
            r#type: "checkbox",
            class: "form-check-input mt-0",
            checked: is_enabled,
            onchange: move |_| props.model.write().toggle_operand(op_id),
          }
        }
        // Operand description & analyzer channel
        td {
          div { class: "d-flex align-items-center gap-1 mb-1",
            span {
              class: "{badge_class}",
              style: "max-width: 140px;",
              "{badge_text}"
            }
          }
          div {
            class: "fw-bold text-light text-truncate",
            style: "max-width: 170px;",
            title: "{detail_text}",
            "{detail_text}"
          }
        }
        // Operator Mode Toggle button (= -> ≤ -> ≥ -> =)
        td { class: "text-center px-1",
          if is_threshold || is_target {
            {
                let (btn_class, title_text) = match op_symbol {
                    "=" => {
                        (
                            "btn-outline-info text-info",
                            "Mode: Target (=). Click to switch to Upper Bound (≤)",
                        )
                    }
                    "≤" => {
                        (
                            "btn-outline-warning text-warning",
                            "Mode: Upper Bound (≤). Click to switch to Lower Bound (≥)",
                        )
                    }
                    _ => {
                        (
                            "btn-outline-warning text-warning",
                            "Mode: Lower Bound (≥). Click to switch to Target (=)",
                        )
                    }
                };
                rsx! {
                  button {
                    class: "btn {btn_class} btn-sm p-0 fw-bold border-secondary font-monospace",
                    style: "width: 28px; height: 22px; line-height: 20px;",
                    title: "{title_text}",
                    disabled: !is_enabled,
                    onclick: move |_| props.model.write().cycle_operand_mode(op_id),
                    "{op_symbol}"
                  }
                }
            }
          } else {
            span { class: "badge bg-secondary font-monospace", "{op_symbol}" }
          }
        }
        // Target/Limit Value Input
        td { class: "px-1",
          OperandNumberInput {
            value: target_val,
            on_commit: move |val| props.model.write().update_operand_target_ui(op_id, val),
          }
        }
        // Weight Input
        td { class: "px-1",
          OperandNumberInput {
            value: weight_val,
            on_commit: move |val| props.model.write().update_operand_weight(op_id, val),
          }
        }
        // Delete
        td { class: "text-end px-1",
          button {
            class: "btn btn-outline-danger btn-sm p-0 border-0",
            style: "line-height: 1;",
            title: "Remove operand",
            onclick: move |_| props.model.write().remove_operand(op_id),
            Icon { icon: FaTrash }
          }
        }
      }
    }
}

/// Tabbed modal dialog to add Targets, Thresholds, Geometric Constraints or Transmission Penalties.
#[component]
pub fn AddOperandDialog(mut model: Signal<OptimizationModel>, mut open: Signal<bool>) -> Element {
    let schema_opt = model.read().schema.clone();
    let mut active_tab = use_signal(|| "observables");

    let grouped_hierarchy = use_memo(move || {
        let schema_opt = model.read().schema.clone();
        let mut hierarchy: DiscoveredObservableHierarchy = Vec::new();
        if let Some(schema) = &schema_opt {
            for obs in &schema.observable_metrics {
                let channel_entry = if let Some(pos) = hierarchy
                    .iter()
                    .position(|(a, _, _)| a == &obs.analyzer_name)
                {
                    &mut hierarchy[pos].2
                } else {
                    hierarchy.push((
                        obs.analyzer_name.clone(),
                        obs.analyzer_kind.clone(),
                        Vec::new(),
                    ));
                    &mut hierarchy.last_mut().unwrap().2
                };

                if let Some(node_pos) = channel_entry.iter().position(|(n, _)| n == &obs.node_name)
                {
                    channel_entry[node_pos].1.push(obs.clone());
                } else {
                    channel_entry.push((obs.node_name.clone(), vec![obs.clone()]));
                }
            }
        }
        hierarchy
    });

    rsx! {
      AlertDialog {
        open: open(),
        on_open_change: move |v| open.set(v),
        max_width: "48rem".to_string(),
        AlertDialogContent {
          div {
            class: "bg-dark text-light p-3 rounded border border-secondary shadow-lg",
            style: "position: relative; z-index: 1060;",

            AlertDialogTitle { "Add Merit Function Operand" }
            AlertDialogDescription {
              div { class: "d-flex flex-column gap-3 text-light mt-2",
                // Tab Navigation
                ul { class: "nav nav-tabs border-secondary small",
                  li { class: "nav-item",
                    button {
                      r#type: "button",
                      class: if active_tab() == "observables" { "nav-link active bg-black text-info border-secondary fw-bold" } else { "nav-link text-muted" },
                      onclick: move |e: MouseEvent| {
                          e.stop_propagation();
                          active_tab.set("observables");
                      },
                      div { class: "d-flex align-items-center gap-1",
                        Icon { icon: FaChartLine }
                        "Observable Metrics"
                      }
                    }
                  }
                  li { class: "nav-item",
                    button {
                      r#type: "button",
                      class: if active_tab() == "geometry" { "nav-link active bg-black text-warning border-secondary fw-bold" } else { "nav-link text-muted" },
                      onclick: move |e: MouseEvent| {
                          e.stop_propagation();
                          active_tab.set("geometry");
                      },
                      div { class: "d-flex align-items-center gap-1",
                        Icon { icon: FaRulerCombined }
                        "Geometric Constraints"
                      }
                    }
                  }
                  li { class: "nav-item",
                    button {
                      r#type: "button",
                      class: if active_tab() == "penalties" { "nav-link active bg-black text-danger border-secondary fw-bold" } else { "nav-link text-muted" },
                      onclick: move |e: MouseEvent| {
                          e.stop_propagation();
                          active_tab.set("penalties");
                      },
                      div { class: "d-flex align-items-center gap-1",
                        Icon { icon: FaFilter }
                        "Transmission & Loss"
                      }
                    }
                  }
                }

                // TAB 1: Observable Metrics (Target vs Threshold)
                if active_tab() == "observables" {
                  ScrollArea { height: "20rem",
                    div { class: "d-flex flex-column gap-2",
                      for (analyzer_name , analyzer_kind , detector_nodes) in grouped_hierarchy.read().iter() {
                        div { class: "border border-secondary rounded overflow-hidden bg-black bg-opacity-75",
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

                          div { class: "p-2 d-flex flex-column gap-2",
                            for (node_name , metrics) in detector_nodes {
                              div { class: "rounded border border-secondary border-opacity-50 bg-dark p-2",
                                div { class: "fw-bold text-light small mb-1 pb-1 border-bottom border-secondary border-opacity-25",
                                  "{node_name}"
                                }
                                div { class: "list-group list-group-flush",
                                  for obs in metrics {
                                    {
                                        let obs_clone = obs.clone();
                                        let scale = default_scale_for_metric(
                                            obs.descriptor.dimension,
                                            &obs.descriptor.name,
                                        );
                                        rsx! {
                                          div { class: "list-group-item bg-transparent text-light py-1 px-2 d-flex justify-content-between align-items-center border-0",
                                            div { class: "font-monospace text-light", "{obs.descriptor.name}" }
                                            button {
                                              r#type: "button",
                                              class: "btn btn-outline-primary btn-sm py-0 px-2 font-monospace",
                                              style: "font-size: 10px;",
                                              title: "Add metric operand (defaults to target = 0.0, mode can be toggled in table)",
                                              onclick: move |e: MouseEvent| {
                                                  e.stop_propagation();
                                                  model
                                                      .write()
                                                      .add_operand(
                                                          StandardOperand::Target(TargetValueOperand {
                                                              name: format!(
                                                                  "{}: {}",
                                                                  obs_clone.node_name,
                                                                  obs_clone.descriptor.name,
                                                              ),
                                                              source: obs_clone.address.clone().into(),
                                                              target: 0.0,
                                                              weight: 1.0,
                                                              scale,
                                                          }),
                                                      );
                                                  open.set(false);
                                              },
                                              "+ Add"
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
                    }
                  }
                }

                // TAB 2: Geometric Constraints (Min Edge Thickness & Total Track Length)
                if active_tab() == "geometry" {
                  ScrollArea { height: "20rem",
                    div { class: "d-flex flex-column gap-3",
                      div { class: "border border-secondary rounded p-2 bg-dark",
                        div { class: "fw-bold text-warning small mb-2 d-flex align-items-center gap-1",
                          Icon { icon: FaLayerGroup }
                          "Min Edge Thickness on Lenses (mm)"
                        }
                        if let Some(schema) = &schema_opt {
                          if !schema.edge_thickness_nodes.is_empty() {
                            div { class: "list-group list-group-flush",
                              for node in &schema.edge_thickness_nodes {
                                {
                                    let node_id = node.node_id;
                                    let node_name = node.node_name.clone();
                                    rsx! {
                                      div { class: "list-group-item bg-transparent text-light py-1 px-2 d-flex justify-content-between align-items-center border-0",
                                        div {
                                          span { class: "fw-bold me-2", "{node_name}" }
                                          span { class: "text-muted small", "({node.node_type})" }
                                        }
                                        button {
                                          r#type: "button",
                                          class: "btn btn-outline-warning btn-sm py-0 px-2 font-monospace",
                                          style: "font-size: 10px;",
                                          title: "Enforces edge thickness >= 1.0 mm to prevent self-intersecting lens bodies",
                                          onclick: move |e: MouseEvent| {
                                              e.stop_propagation();
                                              model
                                                  .write()
                                                  .add_operand(
                                                      StandardOperand::MinEdgeThickness(
                                                          MinEdgeThicknessOperand::new(
                                                              format!("{node_name} Min Edge"),
                                                              node_id,
                                                              1.0e-3,
                                                              5.0,
                                                          ),
                                                      ),
                                                  );
                                              open.set(false);
                                          },
                                          "+ Edge Constraint (≥ 1.0 mm)"
                                        }
                                      }
                                    }
                                }
                              }
                            }
                          } else {
                            div { class: "text-muted small px-2 py-1",
                              "No thick optical components (lenses) with EdgeThickness support found in current setup."
                            }
                          }
                        }
                      }

                      div { class: "border border-secondary rounded p-2 bg-dark d-flex justify-content-between align-items-center",
                        div {
                          div { class: "fw-bold text-warning small",
                            "Overall Track Length Constraint"
                          }
                          div { class: "text-muted small",
                            "Penalizes if the sum of all distance segments exceeds a maximum limit."
                          }
                        }
                        button {
                          r#type: "button",
                          class: "btn btn-outline-warning btn-sm py-1 px-2 font-monospace",
                          style: "font-size: 11px;",
                          onclick: move |e: MouseEvent| {
                              e.stop_propagation();
                              let schema_clone = model.read().schema.clone();
                              if let Some(schema) = schema_clone {
                                  let distance_segments: Vec<ParameterAddress> = schema
                                      .controllable_variables
                                      .into_iter()
                                      .filter_map(|v| match v.address {
                                          ParameterAddress::ConnectionDistance { .. } => Some(v.address),
                                          _ => None,
                                      })
                                      .collect();

                                  if !distance_segments.is_empty() {
                                      model
                                          .write()
                                          .add_operand(
                                              StandardOperand::MaxTotalLength(
                                                  MaxTotalLengthOperand::new(
                                                      "Max Total Length",
                                                      distance_segments,
                                                      0.5,
                                                      1.0,
                                                  ),
                                              ),
                                          );
                                      open.set(false);
                                  }
                              }
                          },
                          "+ Add Track Length Limit"
                        }
                      }
                    }
                  }
                }

                // TAB 3: Transmission & Vignetting Penalties
                if active_tab() == "penalties" {
                  ScrollArea { height: "20rem",
                    div { class: "d-flex flex-column gap-2",
                      p { class: "small text-muted mb-1",
                        "Penalizes ray clipping and energy loss when rays miss apertures along the beam path:"
                      }
                      if let Some(schema) = &schema_opt {
                        for obs in &schema.observable_metrics {
                          if obs.descriptor.name.to_lowercase().contains("rays")
                              || obs.descriptor.name.to_lowercase().contains("valid")
                          {
                            {
                                let obs_pen = obs.clone();
                                rsx! {
                                  div { class: "border border-secondary rounded p-2 bg-dark d-flex justify-content-between align-items-center",
                                    div {
                                      div { class: "fw-bold text-light", "{obs_pen.node_name}" }
                                      div { class: "text-muted font-monospace small",
                                        "{obs_pen.analyzer_name} → {obs_pen.descriptor.name}"
                                      }
                                    }
                                    button {
                                      r#type: "button",
                                      class: "btn btn-outline-danger btn-sm py-0 px-2 font-monospace",
                                      style: "font-size: 10px;",
                                      title: "Penalizes ray losses: weight * ((expected - valid) / expected)^2",
                                      onclick: move |e: MouseEvent| {
                                          e.stop_propagation();
                                          model
                                              .write()
                                              .add_operand(
                                                  StandardOperand::TransmissionPenalty(
                                                      TransmissionPenaltyOperand::new(
                                                          format!("{}: Transmission", obs_pen.node_name),
                                                          obs_pen.address.clone(),
                                                          100,
                                                          2.0,
                                                      ),
                                                  ),
                                              );
                                          open.set(false);
                                      },
                                      "+ Transmission Guard"
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
            }
            AlertDialogActions {
              AlertDialogCancel { on_click: move |_| open.set(false), "Cancel" }
            }
          }
        }
      }
    }
}
