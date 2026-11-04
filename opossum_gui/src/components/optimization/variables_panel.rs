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
    icons::fa_solid_icons::{FaLink, FaPlus, FaSliders, FaTrash, FaXmark},
};
use dioxus_primitives::alert_dialog::AlertDialogContent;
use opossum_optimization::{CouplingType, DiscoveredVariable};

use super::state::OptimizationModel;

#[derive(Props, Clone, PartialEq)]
pub struct VariablesPanelProps {
    pub model: Signal<OptimizationModel>,
}

#[component]
pub fn VariablesPanel(mut props: VariablesPanelProps) -> Element {
    let mut show_add_dialog = use_signal(|| false);
    let mut show_coupling_dialog = use_signal(|| false);

    let rows = props.model.read().variable_rows.clone();

    rsx! {
      div { class: "border rounded p-2 bg-dark text-light h-100 d-flex flex-column",
        div { class: "d-flex justify-content-between align-items-center mb-2 pb-2 border-bottom border-secondary gap-2",
          div { class: "fw-bold small d-flex align-items-center gap-1",
            Icon { icon: FaSliders }
            "Tuned Parameters ({rows.len()})"
          }
          div { class: "d-flex align-items-center gap-2",
            Button {
              variant: ButtonVariant::Primary,
              size: ButtonSize::Sm,
              title: "Pick parameter from model schema",
              onclick: move |_| show_add_dialog.set(true),
              Icon { icon: FaPlus }
              "Variable"
            }
            Button {
              variant: ButtonVariant::Outline,
              size: ButtonSize::Sm,
              disabled: rows.len() < 2,
              title: "Create parameter coupling (e.g. R2 = -R1)",
              onclick: move |_| show_coupling_dialog.set(true),
              Icon { icon: FaLink }
              "Link"
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
                  th { "Node / Property" }
                  th { style: "width: 75px;", "Current" }
                  th { style: "width: 80px;", "Min Bound" }
                  th { style: "width: 80px;", "Max Bound" }
                  th { style: "width: 100px;", "Coupling" }
                  th { style: "width: 25px;", "" }
                }
              }
              tbody {
                for (idx , row) in rows.iter().enumerate() {
                  tr {
                    key: "{row.property_name}_{idx}",
                    class: if !row.is_enabled && row.coupling.is_none() { "opacity-50" } else { "" },

                    // Checkbox: On/Off
                    td { class: "text-center px-1",
                      input {
                        r#type: "checkbox",
                        class: "form-check-input mt-0",
                        checked: row.is_enabled,
                        disabled: row.coupling.is_some(),
                        onchange: {
                            let addr = row.address.clone();
                            move |_| props.model.write().toggle_variable(&addr)
                        },
                      }
                    }

                    // Node & Property
                    td {
                      div {
                        class: "fw-bold text-info text-truncate",
                        style: "max-width: 130px;",
                        title: "{row.node_name}",
                        "{row.node_name}"
                      }
                      div {
                        class: "text-muted text-truncate",
                        style: "max-width: 130px;",
                        "{row.property_name}"
                      }
                    }

                    // Current scalar
                    td { class: "text-end text-light", "{row.current_value:.3}" }

                    // Min Bound
                    td {
                      if row.coupling.is_none() {
                        input {
                          r#type: "number",
                          class: "form-control form-control-sm bg-black text-light border-secondary p-0 px-1 font-monospace text-end",
                          style: "height: 22px; font-size: 11px;",
                          value: "{row.min_bound.unwrap_or(0.0)}",
                          oninput: {
                              let addr = row.address.clone();
                              let max_val = row.max_bound;
                              move |e| {
                                  let new_min = e.value().parse::<f64>().ok();
                                  props.model.write().update_bounds(&addr, new_min, max_val);
                              }
                          },
                        }
                      } else {
                        span { class: "text-muted small", "-" }
                      }
                    }

                    // Max Bound
                    td {
                      if row.coupling.is_none() {
                        input {
                          r#type: "number",
                          class: "form-control form-control-sm bg-black text-light border-secondary p-0 px-1 font-monospace text-end",
                          style: "height: 22px; font-size: 11px;",
                          value: "{row.max_bound.unwrap_or(0.0)}",
                          oninput: {
                              let addr = row.address.clone();
                              let min_val = row.min_bound;
                              move |e| {
                                  let new_max = e.value().parse::<f64>().ok();
                                  props.model.write().update_bounds(&addr, min_val, new_max);
                              }
                          },
                        }
                      } else {
                        span { class: "text-muted small", "-" }
                      }
                    }

                    // Coupling
                    td {
                      if let Some(rule) = &row.coupling {
                        span {
                          class: "badge bg-warning text-dark d-inline-flex align-items-center gap-1 text-truncate",
                          style: "max-width: 95px; cursor: pointer;",
                          title: "Click to remove coupling",
                          onclick: {
                              let addr = row.address.clone();
                              move |_| props.model.write().set_coupling(&addr, None)
                          },
                          Icon { icon: FaLink }
                          match rule {
                              CouplingType::Opposite { .. } => "Opposite",
                              CouplingType::Identical { .. } => "Identical",
                              CouplingType::Affine { .. } => "Affine",
                              CouplingType::TotalLengthLock { .. } => "LengthLock",
                          }
                          Icon { icon: FaXmark }
                        }
                      } else if row.is_enabled {
                        span { class: "badge bg-secondary", "Free Var" }
                      } else {
                        span { class: "text-muted", "Fixed" }
                      }
                    }

                    // Remove Variable Action
                    td { class: "text-end px-1",
                      button {
                        class: "btn btn-outline-danger btn-sm p-0 border-0",
                        style: "line-height: 1;",
                        title: "Remove variable from optimization",
                        onclick: move |_| props.model.write().remove_variable(idx),
                        Icon { icon: FaTrash }
                      }
                    }
                  }
                }

                if rows.is_empty() {
                  tr {
                    td {
                      colspan: "7",
                      class: "text-center text-muted py-4",
                      "No variables selected. Click '+ Variable' to add tuning parameters."
                    }
                  }
                }
              }
            }
          }
        }
      }

      // Dialog: Add Variable from Schema
      if show_add_dialog() {
        AddVariableDialog { model: props.model, open: show_add_dialog }
      }

      // Dialog: Add Coupling
      if show_coupling_dialog() {
        CouplingDialog { model: props.model, open: show_coupling_dialog }
      }
    }
}

/// Dialog allowing user to select controllable variables from the model schema.
#[component]
fn AddVariableDialog(mut model: Signal<OptimizationModel>, mut open: Signal<bool>) -> Element {
    let schema_opt = model.read().schema.clone();
    let current_rows = model.read().variable_rows.clone();

    rsx! {
      AlertDialog {
        open: open(),
        on_open_change: move |v| open.set(v),
        max_width: "42rem".to_string(),
        AlertDialogContent {
          AlertDialogTitle { "Add Tuning Variable" }
          AlertDialogDescription {
            div { class: "d-flex flex-column gap-3 text-light",
              p { class: "small text-muted mb-0",
                "Select a property or connection distance to be adjusted by the solver."
              }

              if let Some(schema) = schema_opt {
                ScrollArea { height: "18rem",
                  div { class: "list-group list-group-flush small",
                    for var in schema.controllable_variables.into_iter().filter(|v| !v.is_read_only) {
                      {
                          let is_already_added = current_rows.iter().any(|r| r.address == var.address);
                          rsx! {
                            button {
                              r#type: "button",
                              class: if is_already_added { "list-group-item bg-dark text-muted border-secondary d-flex justify-content-between align-items-center pe-none opacity-50" } else { "list-group-item list-group-item-action bg-dark text-light border-secondary d-flex justify-content-between align-items-center" },
                              onclick: {
                                  let v_clone = var.clone();
                                  move |_| {
                                      model.write().add_variable_from_schema(&v_clone);
                                      open.set(false);
                                  }
                              },
                              div {
                                div { class: "fw-bold", "{var.node_name}" }
                                div { class: "text-muted font-monospace", "{var.display_name} = {var.initial_value:.3}" }
                              }
                              if is_already_added {
                                span { class: "badge bg-secondary", "Added" }
                              } else {
                                span { class: "badge bg-primary", "+ Add" }
                              }
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

/// Modal dialog to configure a parameter coupling rule.
#[component]
fn CouplingDialog(mut model: Signal<OptimizationModel>, mut open: Signal<bool>) -> Element {
    let vars = model.read().variable_rows.clone();
    let mut target_idx = use_signal(|| 0_usize);
    let mut source_idx = use_signal(|| if vars.len() > 1 { 1 } else { 0 });
    let mut rule_kind = use_signal(|| "Opposite");
    let mut affine_factor = use_signal(|| -1.0_f64);
    let mut affine_offset = use_signal(|| 0.0_f64);

    let create_coupling = move |_| {
        let rows = model.read().variable_rows.clone();
        if let (Some(target), Some(source)) = (rows.get(target_idx()), rows.get(source_idx())) {
            let rule = match *rule_kind.read() {
                "Identical" => CouplingType::Identical {
                    source: source.address.clone(),
                },
                "Affine" => CouplingType::Affine {
                    source: source.address.clone(),
                    factor: affine_factor(),
                    offset: affine_offset(),
                },
                _ => CouplingType::Opposite {
                    source: source.address.clone(),
                },
            };
            model.write().set_coupling(&target.address, Some(rule));
            open.set(false);
        }
    };

    rsx! {
      AlertDialog {
        open: open(),
        on_open_change: move |v| open.set(v),
        max_width: "42rem".to_string(),
        AlertDialogContent {
          AlertDialogTitle { "Create Parameter Coupling" }
          AlertDialogDescription {
            div { class: "d-flex flex-column gap-3 text-light",
              div {
                label { class: "form-label small text-warning mb-1",
                  "Dependent Target Parameter"
                }
                select {
                  class: "form-select form-select-sm bg-black text-light border-secondary",
                  value: "{target_idx}",
                  onchange: move |e| target_idx.set(e.value().parse().unwrap_or(0)),
                  for (i , v) in vars.iter().enumerate() {
                    option { value: "{i}", "{v.node_name} — {v.property_name}" }
                  }
                }
              }
              div {
                label { class: "form-label small text-info mb-1", "Coupling Rule" }
                select {
                  class: "form-select form-select-sm bg-black text-light border-secondary",
                  value: "{rule_kind}",
                  onchange: move |e| {
                      rule_kind
                          .set(
                              match e.value().as_str() {
                                  "Identical" => "Identical",
                                  "Affine" => "Affine",
                                  _ => "Opposite",
                              },
                          )
                  },
                  option { value: "Opposite",
                    "Opposite: Target = -1.0 × Source (e.g. Symmetrical Lens R2 = -R1)"
                  }
                  option { value: "Identical",
                    "Identical: Target = Source (e.g. Equidistant Spacings)"
                  }
                  option { value: "Affine", "Affine: Target = factor × Source + offset" }
                }
              }
              div {
                label { class: "form-label small text-info mb-1",
                  "Driving Source Variable"
                }
                select {
                  class: "form-select form-select-sm bg-black text-light border-secondary",
                  value: "{source_idx}",
                  onchange: move |e| source_idx.set(e.value().parse().unwrap_or(0)),
                  for (i , v) in vars.iter().enumerate() {
                    if i != target_idx() {
                      option { value: "{i}", "{v.node_name} — {v.property_name}" }
                    }
                  }
                }
              }
            }
          }
          AlertDialogActions {
            AlertDialogCancel { on_click: move |_| open.set(false), "Cancel" }
            Button {
              variant: ButtonVariant::Primary,
              size: ButtonSize::Sm,
              onclick: create_coupling,
              "Add Coupling"
            }
          }
        }
      }
    }
}