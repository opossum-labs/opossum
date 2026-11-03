use crate::components::primitives::{
    button::{Button, ButtonSize, ButtonVariant},
    scroll_area::ScrollArea,
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaLink, FaPlus, FaSliders},
};
use super::modal::{MockCoupling, MockVariable};

#[derive(Props, Clone, PartialEq)]
pub struct VariablesPanelProps {
    pub variables: Signal<Vec<MockVariable>>,
    pub couplings: Signal<Vec<MockCoupling>>,
}

#[component]
pub fn VariablesPanel(props: VariablesPanelProps) -> Element {
    rsx! {
      div { class: "border rounded p-3 bg-dark text-light h-100 d-flex flex-column",
        // Section Header: Free Variables
        div { class: "d-flex justify-content-between align-items-center mb-2 pb-2 border-bottom border-secondary",
          div { class: "fw-bold small d-flex align-items-center gap-2",
            Icon { icon: FaSliders }
            "Tuned Variables ({props.variables.read().len()})"
          }
          Button {
            variant: ButtonVariant::Outline,
            size: ButtonSize::Sm,
            title: "Pick variable from optical scenery tree",
            Icon { icon: FaPlus }
            "Add Variable"
          }
        }

        // Scrollable list of active variables
        ScrollArea { height: "18rem",
          div { class: "d-flex flex-column gap-2 pe-2",
            for var in props.variables.read().iter() {
              div {
                key: "{var.id}",
                class: "p-2 rounded bg-opacity-25 bg-secondary border border-secondary",
                div { class: "d-flex justify-content-between align-items-center",
                  span { class: "fw-bold text-info small", "{var.node_name}" }
                  span { class: "badge bg-dark font-monospace",
                    "{var.current_value:.2} {var.unit}"
                  }
                }
                div { class: "text-muted small mb-2", "{var.property_name}" }

                // Min / Max Bounds inputs
                div { class: "row g-2 align-items-center",
                  div { class: "col-6",
                    div { class: "input-group input-group-sm",
                      span { class: "input-group-text bg-dark text-muted border-secondary",
                        "Min"
                      }
                      input {
                        r#type: "number",
                        class: "form-control form-control-sm bg-dark text-light border-secondary font-monospace",
                        value: "{var.min_bound}",
                      }
                    }
                  }
                  div { class: "col-6",
                    div { class: "input-group input-group-sm",
                      span { class: "input-group-text bg-dark text-muted border-secondary",
                        "Max"
                      }
                      input {
                        r#type: "number",
                        class: "form-control form-control-sm bg-dark text-light border-secondary font-monospace",
                        value: "{var.max_bound}",
                      }
                    }
                  }
                }
              }
            }
          }
        }

        // Section Header: Couplings
        div { class: "d-flex justify-content-between align-items-center mt-3 mb-2 pb-2 border-bottom border-secondary",
          div { class: "fw-bold small d-flex align-items-center gap-2",
            Icon { icon: FaLink }
            "Parameter Couplings ({props.couplings.read().len()})"
          }
          Button {
            variant: ButtonVariant::Outline,
            size: ButtonSize::Sm,
            title: "Add symmetrical or affine dependency",
            Icon { icon: FaPlus }
            "Link"
          }
        }

        // Couplings list
        ScrollArea { height: "7rem",
          div { class: "d-flex flex-column gap-2 pe-2",
            for (i , c) in props.couplings.read().iter().enumerate() {
              div {
                key: "{i}",
                class: "p-2 rounded border border-secondary bg-black bg-opacity-25 small",
                div { class: "d-flex align-items-center justify-content-between",
                  span { class: "text-warning fw-bold",
                    "{c.target_node} → {c.target_prop}"
                  }
                  span { class: "badge bg-secondary", "{c.rule_desc}" }
                }
                div { class: "text-muted small mt-1",
                  "Driven by: "
                  span { class: "text-light", "{c.source_node} ({c.source_prop})" }
                }
              }
            }
          }
        }
      }
    }
}