use super::{
    numeric_input::NumericCommitInput,
    state::{OptimizationModel, UiVariableRow},
};
use crate::components::primitives::{
    button::{Button, ButtonSize, ButtonVariant},
    scroll_area::ScrollArea,
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaLayerGroup, FaLink, FaPlus, FaSliders, FaTrash, FaXmark},
};
use opossum_core::opm_document::optimization::CouplingType;

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
