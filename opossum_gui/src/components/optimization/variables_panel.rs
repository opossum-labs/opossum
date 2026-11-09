use crate::components::{
    optimization::state::UiVariableRow,
    primitives::{
        alert_dialog::{
            AlertDialog, AlertDialogActions, AlertDialogCancel, AlertDialogDescription,
            AlertDialogTitle,
        },
        button::{Button, ButtonSize, ButtonVariant},
        scroll_area::ScrollArea,
    },
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaLayerGroup, FaLink, FaPlus, FaSliders, FaTrash, FaXmark},
};
use dioxus_primitives::alert_dialog::AlertDialogContent;
use opossum_optimization::{CouplingType, DiscoveredVariable, ParameterAddress};

use super::state::OptimizationModel;

#[derive(Props, Clone, PartialEq)]
pub struct VariablesPanelProps {
    pub model: Signal<OptimizationModel>,
    pub show_add_dialog: Signal<bool>,
    pub show_coupling_dialog: Signal<bool>,
}

#[component]
pub fn VariablesPanel(mut props: VariablesPanelProps) -> Element {
    let rows = props.model.read().variable_rows.clone();

    // Group rows by group_name while keeping the original row index for mutations
    let mut grouped_vars: Vec<(String, Vec<(usize, UiVariableRow)>)> = Vec::new();
    for (idx, row) in rows.iter().enumerate() {
        if let Some((_, list)) = grouped_vars
            .iter_mut()
            .find(|(name, _)| name == &row.group_name)
        {
            list.push((idx, row.clone()));
        } else {
            grouped_vars.push((row.group_name.clone(), vec![(idx, row.clone())]));
        }
    }

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
              onclick: move |_| props.show_add_dialog.set(true),
              Icon { icon: FaPlus }
              "Variable"
            }
            Button {
              variant: ButtonVariant::Outline,
              size: ButtonSize::Sm,
              disabled: rows.len() < 2,
              title: "Create parameter coupling (e.g. R2 = -R1)",
              onclick: move |_| props.show_coupling_dialog.set(true),
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
              thead {
                class: "table-active sticky-top",
                style: "z-index: 1;",
                tr {
                  th { style: "width: 25px;", "" }
                  th { "Node / Property" }
                  th { style: "width: 95px;", "Current" }
                  th { style: "width: 75px;", "Min" }
                  th { style: "width: 75px;", "Max" }
                  th { style: "width: 95px;", "Coupling" }
                  th { style: "width: 25px;", "" }
                }
              }
              tbody {
                for (group_name , group_rows) in grouped_vars.iter() {
                  // Group Header Banner
                  tr { class: "table-secondary bg-black bg-opacity-75 border-top border-secondary",
                    td {
                      colspan: "7",
                      class: "py-1 px-2 text-info fw-bold small text-uppercase",
                      div { class: "d-flex align-items-center gap-2",
                        Icon {
                          icon: FaLayerGroup,
                          class: "text-warning",
                        }
                        span { "{group_name}" }
                        span {
                          class: "badge bg-secondary rounded-pill font-monospace",
                          style: "font-size: 9px;",
                          "{group_rows.len()} vars"
                        }
                      }
                    }
                  }

                  // Rows belonging to this group
                  for & (idx , ref row) in group_rows.iter() {
                    {
                        let (main_val, sub_val) = row.display_current();
                        rsx! {
                          tr {
                            key: "{row.property_name}_{idx}",
                            class: if !row.is_enabled && row.coupling.is_none() { "opacity-50" } else { "" },

                            // Checkbox
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

                            // Node & Property with indentation
                            td { class: "ps-3",
                              div {
                                class: "fw-bold text-light text-truncate",
                                style: "max-width: 140px;",
                                title: "{row.node_name}",
                                "{row.node_name}"
                              }
                              div { class: "text-muted text-truncate", style: "max-width: 140px;", "{row.property_name}" }
                            }

                            // Current Value
                            td { class: "text-end px-1",
                              div { class: "text-light fw-bold", "{main_val}" }
                              if let Some(sub) = sub_val {
                                div { class: "text-muted", style: "font-size: 9px;", "{sub}" }
                              }
                            }

                            // Min Bound
                            td { class: "px-1",
                              if row.coupling.is_none() {
                                BoundInput {
                                  value: row.min_bound,
                                  on_commit: {
                                      let addr = row.address.clone();
                                      move |val| props.model.write().set_min_bound(&addr, val)
                                  },
                                }
                              } else {
                                span { class: "text-muted small", "-" }
                              }
                            }

                            // Max Bound
                            td { class: "px-1",
                              if row.coupling.is_none() {
                                BoundInput {
                                  value: row.max_bound,
                                  on_commit: {
                                      let addr = row.address.clone();
                                      move |val| props.model.write().set_max_bound(&addr, val)
                                  },
                                }
                              } else {
                                span { class: "text-muted small", "-" }
                              }
                            }

                            // Coupling Status
                            td { class: "px-1",
                              if let Some(rule) = &row.coupling {
                                span {
                                  class: "badge bg-warning text-dark d-inline-flex align-items-center gap-1 text-truncate",
                                  style: "max-width: 90px; cursor: pointer;",
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

                            // Delete Action
                            td { class: "text-end px-1",
                              button {
                                class: "btn btn-outline-danger btn-sm p-0 border-0",
                                style: "line-height: 1;",
                                title: "Remove variable",
                                onclick: move |_| props.model.write().remove_variable(idx),
                                Icon { icon: FaTrash }
                              }
                            }
                          }
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
    }
}

/// Dialog allowing user to select controllable variables from the model schema grouped by NodeGroup and Node.
#[component]
pub fn AddVariableDialog(mut model: Signal<OptimizationModel>, mut open: Signal<bool>) -> Element {
    let schema_opt = model.read().schema.clone();
    let current_rows = model.read().variable_rows.clone();

    // Grouping: group_name -> (node_name -> Vec<DiscoveredVariable>)
    let mut grouped_hierarchy: Vec<(String, Vec<(String, Vec<DiscoveredVariable>)>)> = Vec::new();

    if let Some(schema) = &schema_opt {
        for var in schema
            .controllable_variables
            .iter()
            .filter(|v| !v.is_read_only)
        {
            // 1. Group level
            let group_entry = if let Some(pos) = grouped_hierarchy
                .iter()
                .position(|(g, _)| g == &var.group_name)
            {
                &mut grouped_hierarchy[pos].1
            } else {
                grouped_hierarchy.push((var.group_name.clone(), Vec::new()));
                &mut grouped_hierarchy.last_mut().unwrap().1
            };

            // 2. Node level
            if let Some(node_pos) = group_entry.iter().position(|(n, _)| n == &var.node_name) {
                group_entry[node_pos].1.push(var.clone());
            } else {
                group_entry.push((var.node_name.clone(), vec![var.clone()]));
            }
        }
    }

    rsx! {
      AlertDialog {
        open: open(),
        on_open_change: move |v| open.set(v),
        max_width: "46rem".to_string(),
        AlertDialogContent {
          div {
            class: "bg-dark text-light p-3 rounded border border-secondary shadow-lg",
            style: "position: relative; z-index: 1060;",

            AlertDialogTitle { "Add Tuning Variable" }
            AlertDialogDescription {
              div { class: "d-flex flex-column gap-3 text-light mt-2",
                p { class: "small text-muted mb-0",
                  "Select optical parameters organized by group and component:"
                }

                if !grouped_hierarchy.is_empty() {
                  ScrollArea { height: "22rem",
                    div { class: "d-flex flex-column gap-2",
                      for (group_name , node_list) in grouped_hierarchy {
                        div { class: "border border-secondary rounded overflow-hidden bg-black bg-opacity-50",
                          // Level 1: NodeGroup Header
                          div { class: "bg-black py-1 px-2 text-warning fw-bold small d-flex align-items-center gap-2 border-bottom border-secondary text-uppercase",
                            Icon { icon: FaLayerGroup }
                            span { "{group_name}" }
                          }

                          // Level 2: Component Nodes
                          div { class: "p-2 d-flex flex-column gap-2",
                            for (node_name , vars) in node_list {
                              div { class: "rounded border border-secondary border-opacity-50 bg-dark p-2",
                                div { class: "fw-bold text-info small mb-1 pb-1 border-bottom border-secondary border-opacity-25 d-flex align-items-center justify-content-between",
                                  span { "{node_name}" }
                                  span {
                                    class: "badge bg-secondary font-monospace",
                                    style: "font-size: 9px;",
                                    "{vars.len()} props"
                                  }
                                }

                                div { class: "list-group list-group-flush",
                                  for var in vars {
                                    {
                                        let is_already_added = current_rows.iter().any(|r| r.address == var.address);
                                        let is_curv = matches!(var.address, ParameterAddress::SurfaceCurvature { .. });
                                        let val_display = if is_curv {
                                            if var.initial_value.abs() < 1e-9 {
                                                "Plan (R = ∞)".to_string()
                                            } else {
                                                format!("R = {:.1} mm", 1000.0 / var.initial_value)
                                            }
                                        } else if matches!(var.address, ParameterAddress::ConnectionDistance { .. }) {
                                            format!("{:.2} mm", var.initial_value * 1000.0)
                                        } else {
                                            format!("{:.3}", var.initial_value)
                                        };

                                        rsx! {
                                          button {
                                            r#type: "button",
                                            class: if is_already_added { "list-group-item bg-transparent text-muted py-1 px-2 d-flex justify-content-between align-items-center pe-none opacity-50 border-0" } else { "list-group-item list-group-item-action bg-transparent text-light py-1 px-2 d-flex justify-content-between align-items-center rounded border-0" },
                                            onclick: {
                                                let v_clone = var.clone();
                                                move |_| {
                                                    model.write().add_variable_from_schema(&v_clone);
                                                    open.set(false);
                                                }
                                            },
                                            div { class: "d-flex align-items-center gap-2",
                                              span { class: "font-monospace text-light", "{var.display_name}" }
                                              span { class: "text-muted font-monospace small", "({val_display})" }
                                            }
                                            if is_already_added {
                                              span { class: "badge bg-secondary", style: "font-size: 9px;", "Added" }
                                            } else {
                                              span { class: "badge bg-primary", style: "font-size: 9px;", "+ Add" }
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
}

/// Modal dialog to configure a parameter coupling rule.
#[component]
pub fn CouplingDialog(mut model: Signal<OptimizationModel>, mut open: Signal<bool>) -> Element {
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
          div {
            class: "bg-dark text-light p-3 rounded border border-secondary shadow-lg",
            style: "position: relative; z-index: 1060;",

            AlertDialogTitle { "Create Parameter Coupling" }
            AlertDialogDescription {
              div { class: "d-flex flex-column gap-3 text-light mt-2",
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
                  label { class: "form-label small text-info mb-1",
                    "Coupling Rule"
                  }
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
                    option { value: "Affine",
                      "Affine: Target = factor × Source + offset"
                    }
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
                        option { value: "{i}",
                          "{v.node_name} — {v.property_name}"
                        }
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
}

/// Local controlled text input for numeric bounds.
/// Commits value changes only on blur or when the Enter key is pressed,
/// formatting to up to 3 decimal places without redundant trailing zeros.
#[component]
fn BoundInput(value: Option<f64>, on_commit: EventHandler<Option<f64>>) -> Element {
    let format_val = |v: Option<f64>| {
        v.map_or_else(String::new, |num| {
            let s = format!("{num:.3}");
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
        })
    };

    let mut text = use_signal(|| format_val(value));
    let mut last_external = use_signal(|| value);

    // Synchronize if the bound was updated externally (e.g. from Rule Assistants)
    if *last_external.read() != value {
        last_external.set(value);
        text.set(format_val(value));
    }

    let mut commit = move || {
        let raw = text.read().trim().to_string();
        if raw.is_empty() {
            last_external.set(None);
            text.set(String::new());
            on_commit.call(None);
        } else if let Ok(parsed) = raw.parse::<f64>() {
            last_external.set(Some(parsed));
            text.set(format_val(Some(parsed)));
            on_commit.call(Some(parsed));
        } else {
            // Revert invalid strings back to last valid value
            text.set(format_val(*last_external.read()));
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