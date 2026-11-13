use super::{numeric_input::NumericCommitInput, state::OptimizationModel};
use crate::components::primitives::{
    button::{Button, ButtonSize, ButtonVariant},
    scroll_area::ScrollArea,
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaBullseye, FaPlus, FaTrash},
};
use opossum_core::opm_document::optimization::StandardOperand;
use uuid::Uuid;

#[derive(Props, Clone, PartialEq, Eq)]
pub struct MeritDesignerProps {
    pub model: Signal<OptimizationModel>,
    pub is_running: bool,
    pub show_add_dialog: Signal<bool>,
}

#[component]
pub fn MeritDesignerPanel(mut props: MeritDesignerProps) -> Element {
    let operand_ids: Vec<Uuid> = props.model.read().operands.iter().map(|op| op.id).collect();
    let operands_count = operand_ids.len();

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
            disabled: props.is_running,
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
                  th { style: "width: 25px;", "" }
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
                for op_id in operand_ids {
                  OperandRow {
                    key: "{op_id}",
                    operand_id: op_id,
                    model: props.model,
                    is_running: props.is_running,
                  }
                }
                if operands_count == 0 {
                  tr {
                    td {
                      colspan: "6",
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

#[derive(Props, Clone, PartialEq)]
struct OperandRowProps {
    operand_id: Uuid,
    model: Signal<OptimizationModel>,
    is_running: bool,
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
        StandardOperand::Metric(m) => (m.target, m.weight),
        StandardOperand::MinEdgeThickness(t) => (t.min_thickness * 1000.0, t.weight),
        StandardOperand::MaxTotalLength(t) => (t.max_length * 1000.0, t.weight),
        StandardOperand::TransmissionPenalty(t) => (t.expected_rays, t.weight),
    };
    let is_metric = matches!(op, StandardOperand::Metric(_));
    let badge_class = if badge_text == "Geometry" {
        "badge bg-warning text-dark font-sans"
    } else {
        "badge bg-primary text-light font-sans text-truncate"
    };
    drop(model_read);

    rsx! {
      tr { class: if is_enabled { "" } else { "opacity-50" },
        // Checkbox column
        td { class: "text-center px-1",
          input {
            r#type: "checkbox",
            class: "form-check-input mt-0",
            checked: is_enabled,
            disabled: props.is_running,
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
          if is_metric {
            button {
              class: "btn btn-outline-info btn-sm p-0 fw-bold border-secondary font-monospace",
              style: "width: 28px; height: 22px; line-height: 20px;",
              title: "Click to cycle mode (= -> ≤ -> ≥)",
              disabled: props.is_running || !is_enabled,
              onclick: move |_| props.model.write().cycle_operand_mode(op_id),
              "{op_symbol}"
            }
          } else {
            span { class: "badge bg-secondary font-monospace", "{op_symbol}" }
          }
        }
        // Target/Limit Value Input
        td { class: "px-1",
          NumericCommitInput {
            value: Some(target_val),
            disabled: props.is_running,
            precision: 4,
            allow_empty: false,
            on_commit: move |opt: Option<f64>| {
                if let Some(val) = opt {
                    props.model.write().update_operand_target_ui(op_id, val);
                }
            },
          }
        }
        // Weight Input
        td { class: "px-1",
          NumericCommitInput {
            value: Some(weight_val),
            disabled: props.is_running,
            precision: 4,
            allow_empty: false,
            on_commit: move |opt: Option<f64>| {
                if let Some(val) = opt {
                    props.model.write().update_operand_weight(op_id, val);
                }
            },
          }
        }
        // Delete
        td { class: "text-end px-1",
          button {
            class: "btn btn-outline-danger btn-sm p-0 border-0",
            style: "line-height: 1;",
            disabled: props.is_running,
            title: "Remove operand",
            onclick: move |_| props.model.write().remove_operand(op_id),
            Icon { icon: FaTrash }
          }
        }
      }
    }
}
