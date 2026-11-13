use super::super::state::{OptimizationModel, format_parameter_value};
use crate::components::primitives::{
    alert_dialog::{
        AlertDialog, AlertDialogActions, AlertDialogCancel, AlertDialogDescription,
        AlertDialogTitle,
    },
    scroll_area::ScrollArea,
};
use dioxus::prelude::*;
use dioxus_free_icons::{Icon, icons::fa_solid_icons::FaLayerGroup};
use dioxus_primitives::alert_dialog::AlertDialogContent;
use opossum_core::opm_document::optimization::DiscoveredVariable;

type DiscoveredVariableHierarchy = Vec<(String, Vec<(String, Vec<DiscoveredVariable>)>)>;

/// Modal dialog allowing the user to pick controllable variables from the model schema,
/// grouped hierarchically by `NodeGroup` and optical component node.
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
