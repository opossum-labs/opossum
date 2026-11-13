use super::super::state::OptimizationModel;
use crate::components::primitives::{
    alert_dialog::{
        AlertDialog, AlertDialogActions, AlertDialogCancel, AlertDialogDescription,
        AlertDialogTitle,
    },
    button::{Button, ButtonSize, ButtonVariant},
};
use dioxus::prelude::*;
use dioxus_primitives::alert_dialog::AlertDialogContent;
use opossum_core::opm_document::optimization::CouplingType;

/// Modal dialog to configure a parameter coupling rule (`Opposite`, `Identical`, or `Affine`).
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
