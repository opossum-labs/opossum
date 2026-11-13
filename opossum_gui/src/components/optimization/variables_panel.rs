use super::{
    numeric_input::NumericCommitInput,
    state::{OptimizationModel, UiVariableRow, format_parameter_value},
};
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
    icons::fa_solid_icons::{FaLayerGroup, FaLink, FaPlus, FaSliders, FaTrash, FaXmark},
};
use dioxus_primitives::alert_dialog::AlertDialogContent;
use opossum_core::opm_document::optimization::CouplingType;
use opossum_optimization::DiscoveredVariable;

type DiscoveredVariableHierarchy = Vec<(String, Vec<(String, Vec<DiscoveredVariable>)>)>;

#[derive(Props, Clone, PartialEq, Eq)]
pub struct VariablesPanelProps {
    pub model: Signal<OptimizationModel>,
    pub is_running: bool,
    pub show_add_dialog: Signal<bool>,
    pub show_coupling_dialog: Signal<bool>,
}

#[component]
pub fn VariablesPanel(mut props: VariablesPanelProps) -> Element {
    let grouped_vars = use_memo(move || {
        let rows = &props.model.read().variable_rows;
        let mut groups: Vec<(String, Vec<UiVariableRow>)> = Vec::new();
        for row in rows {
            if let Some((_, list)) = groups.iter_mut().find(|(name, _)| name == &row.group_name) {
                list.push(row.clone());
            } else {
                groups.push((row.group_name.clone(), vec![row.clone()]));
            }
        }
        groups
    });
    let total_vars = use_memo(move || props.model.read().variable_rows.len());

    rsx! {
      div { class: "border rounded p-2 bg-dark text-light h-100 d-flex flex-column",
        div { class: "d-flex justify-content-between align-items-center mb-2 pb-2 border-bottom border-secondary gap-2",
          div { class: "fw-bold small d-flex align-items-center gap-1",
            Icon { icon: FaSliders }
            "Tuned Parameters ({total_vars})"
          }
          div { class: "d-flex align-items-center gap-2",
            Button {
              variant: ButtonVariant::Primary,
              size: ButtonSize::Sm,
              disabled: props.is_running,
              title: "Pick parameter from model schema",
              onclick: move |_| props.show_add_dialog.set(true),
              Icon { icon: FaPlus }
              "Variable"
            }
            Button {
              variant: ButtonVariant::Outline,
              size: ButtonSize::Sm,
              disabled: props.is_running || total_vars() < 2,
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
                for (group_name , group_rows) in grouped_vars.read().iter() {
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
                  for row in group_rows.iter() {
                    {
                        let (main_val, sub_val) = row.display_current();
                        rsx! {
                          tr {
                            key: "{row.address:?}",
                            class: if !row.is_enabled && row.coupling.is_none() { "opacity-50" } else { "" },
                            // Checkbox
                            td { class: "text-center px-1",
                              input {
                                r#type: "checkbox",
                                class: "form-check-input mt-0",
                                checked: row.is_enabled,
                                disabled: props.is_running || row.coupling.is_some(),
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
                                NumericCommitInput {
                                  value: row.min_bound,
                                  disabled: props.is_running,
                                  precision: 3,
                                  allow_empty: true,
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
                                NumericCommitInput {
                                  value: row.max_bound,
                                  disabled: props.is_running,
                                  precision: 3,
                                  allow_empty: true,
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
                                  class: if props.is_running { "badge bg-warning text-dark d-inline-flex align-items-center gap-1 text-truncate opacity-50" } else { "badge bg-warning text-dark d-inline-flex align-items-center gap-1 text-truncate" },
                                  style: if props.is_running { "max-width: 90px; cursor: not-allowed;" } else { "max-width: 90px; cursor: pointer;" },
                                  title: if props.is_running { "Locked while optimization is running" } else { "Click to remove coupling" },
                                  onclick: {
                                      let addr = row.address.clone();
                                      let is_running = props.is_running;
                                      move |_| {
                                          if !is_running {
                                              props.model.write().set_coupling(&addr, None);
                                          }
                                      }
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
                                disabled: props.is_running,
                                title: "Remove variable",
                                onclick: {
                                    let addr = row.address.clone();
                                    move |_| props.model.write().remove_variable(&addr)
                                },
                                Icon { icon: FaTrash }
                              }
                            }
                          }
                        }
                    }
                  }
                }
                if total_vars() == 0 {
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

/// Dialog allowing user to select controllable variables from the model schema grouped by `NodeGroup` and Node.
#[component]
pub fn AddVariableDialog(mut model: Signal<OptimizationModel>, mut open: Signal<bool>) -> Element {
    let grouped_hierarchy = use_memo(move || {
        let schema_read = model.read();
        let mut hierarchy: DiscoveredVariableHierarchy = Vec::new();
        if let Some(schema) = &schema_read.schema {
            for var in schema
                .controllable_variables
                .iter()
                .filter(|v| !v.is_read_only)
            {
                let group_entry =
                    if let Some(pos) = hierarchy.iter().position(|(g, _)| g == &var.group_name) {
                        &mut hierarchy[pos].1
                    } else {
                        hierarchy.push((var.group_name.clone(), Vec::new()));
                        &mut hierarchy.last_mut().unwrap().1
                    };

                if let Some(node_pos) = group_entry.iter().position(|(n, _)| n == &var.node_name) {
                    group_entry[node_pos].1.push(var.clone());
                } else {
                    group_entry.push((var.node_name.clone(), vec![var.clone()]));
                }
            }
        }
        hierarchy
    });

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
                if !grouped_hierarchy.read().is_empty() {
                  ScrollArea { height: "22rem",
                    div { class: "d-flex flex-column gap-2",
                      for (group_name , node_list) in grouped_hierarchy.read().iter() {
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
                                        let is_already_added = model
                                            .read()
                                            .variable_rows
                                            .iter()
                                            .any(|r| r.address == var.address);
                                        let (val_display, _) = format_parameter_value(&var.address, var.initial_value);
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
    let vars_labels: Vec<(String, String)> = model
        .read()
        .variable_rows
        .iter()
        .map(|v| (v.node_name.clone(), v.property_name.clone()))
        .collect();
    let var_count = vars_labels.len();

    let mut target_idx = use_signal(|| 0_usize);
    let mut source_idx = use_signal(|| usize::from(var_count > 1));
    let mut rule_kind = use_signal(|| "Opposite");
    let mut affine_factor = use_signal(|| -1.0_f64);
    let mut affine_offset = use_signal(|| 0.0_f64);

    let create_coupling = move |_| {
        let coupling_payload = {
            let model_read = model.read();
            let rows = &model_read.variable_rows;
            if let (Some(target), Some(source)) = (rows.get(target_idx()), rows.get(source_idx()))
                && target.address != source.address
            {
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
                Some((target.address.clone(), rule))
            } else {
                None
            }
        };

        if let Some((target_addr, rule)) = coupling_payload {
            model.write().set_coupling(&target_addr, Some(rule));
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
                    onchange: move |e| {
                        let next_target = e.value().parse().unwrap_or(0);
                        target_idx.set(next_target);
                        if next_target == source_idx() && var_count > 1 {
                            source_idx.set(usize::from(next_target == 0));
                        }
                    },
                    for (i , (node_name , prop_name)) in vars_labels.iter().enumerate() {
                      option { value: "{i}", "{node_name} — {prop_name}" }
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
                            );
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
                if *rule_kind.read() == "Affine" {
                  div { class: "row g-2 p-2 rounded bg-black bg-opacity-50 border border-secondary",
                    div { class: "col-6",
                      label { class: "form-label small text-info mb-1",
                        "Scale Factor"
                      }
                      input {
                        r#type: "number",
                        step: "any",
                        class: "form-control form-control-sm bg-black text-light border-secondary font-monospace text-end",
                        value: "{affine_factor}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<f64>() {
                                affine_factor.set(v);
                            }
                        },
                      }
                    }
                    div { class: "col-6",
                      label { class: "form-label small text-info mb-1",
                        "Offset"
                      }
                      input {
                        r#type: "number",
                        step: "any",
                        class: "form-control form-control-sm bg-black text-light border-secondary font-monospace text-end",
                        value: "{affine_offset}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<f64>() {
                                affine_offset.set(v);
                            }
                        },
                      }
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
                    for (i , (node_name , prop_name)) in vars_labels.iter().enumerate() {
                      if i != target_idx() {
                        option { value: "{i}", "{node_name} — {prop_name}" }
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
