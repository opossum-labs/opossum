use crate::components::primitives::{
    button::{Button, ButtonSize, ButtonVariant},
    scroll_area::ScrollArea,
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaChevronDown, FaLink, FaMagnifyingGlass, FaPlus, FaSliders},
};
use opossum_optimization::CouplingType;

use super::state::OptimizationModel;

#[derive(Props, Clone, PartialEq)]
pub struct VariablesPanelProps {
    pub model: Signal<OptimizationModel>,
}

#[component]
pub fn VariablesPanel(mut props: VariablesPanelProps) -> Element {
    let mut search_query = use_signal(String::new);

    let rows = props.model.read().variable_rows.clone();
    let query = search_query.read().to_lowercase();

    let filtered_rows: Vec<_> = rows
        .into_iter()
        .filter(|r| {
            if query.is_empty() {
                true
            } else {
                r.node_name.to_lowercase().contains(&query)
                    || r.property_name.to_lowercase().contains(&query)
                    || r.group_name.to_lowercase().contains(&query)
            }
        })
        .collect();

    rsx! {
      div { class: "border rounded p-2 bg-dark text-light h-100 d-flex flex-column",
        div { class: "d-flex justify-content-between align-items-center mb-2 pb-2 border-bottom border-secondary gap-2",
          div { class: "fw-bold small d-flex align-items-center gap-1",
            Icon { icon: FaSliders }
            "Parameters ({filtered_rows.len()})"
          }
          div {
            class: "input-group input-group-sm",
            style: "max-width: 14rem;",
            span { class: "input-group-text bg-black text-muted border-secondary",
              Icon { icon: FaMagnifyingGlass }
            }
            input {
              class: "form-control form-control-sm bg-black text-light border-secondary",
              placeholder: "Filter node / property...",
              value: "{search_query}",
              oninput: move |e| search_query.set(e.value()),
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
                  th { style: "width: 110px;", "Coupling / Rule" }
                }
              }
              tbody {
                for row in filtered_rows {
                  tr {
                    key: "{row.property_name}",
                    class: if !row.is_enabled && row.coupling.is_none() { "opacity-50" } else { "" },
                    // Checkbox: Free Variable On/Off
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
                    // Node & Property Name
                    td {
                      div {
                        class: "fw-bold text-info text-truncate",
                        style: "max-width: 140px;",
                        title: "{row.node_name}",
                        "{row.node_name}"
                      }
                      div { class: "text-muted", "{row.property_name}" }
                    }
                    // Current scalar value
                    td { class: "text-end text-light", "{row.current_value:.3}" }
                    // Min Bound Input
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
                    // Max Bound Input
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
                    // Coupling Status / Action
                    td {
                      if let Some(rule) = &row.coupling {
                        span {
                          class: "badge bg-warning text-dark d-inline-flex align-items-center gap-1 text-truncate",
                          style: "max-width: 105px;",
                          Icon { icon: FaLink }
                          match rule {
                              CouplingType::Opposite { .. } => "Opposite",
                              CouplingType::Identical { .. } => "Identical",
                              CouplingType::Affine { .. } => "Affine",
                              CouplingType::TotalLengthLock { .. } => "LengthLock",
                          }
                        }
                      } else if row.is_enabled {
                        span { class: "text-muted", "Free Var" }
                      } else {
                        span { class: "text-muted", "Fixed" }
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