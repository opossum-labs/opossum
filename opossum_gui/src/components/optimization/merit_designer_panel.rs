use crate::components::primitives::{
    button::{Button, ButtonSize, ButtonVariant},
    scroll_area::ScrollArea,
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaCheck, FaCube, FaEye, FaPlus, FaRulerCombined, FaTrash},
};
use super::modal::MockOperand;

#[derive(Props, Clone, PartialEq)]
pub struct MeritDesignerProps {
    pub operands: Signal<Vec<MockOperand>>,
}

#[component]
pub fn MeritDesignerPanel(props: MeritDesignerProps) -> Element {
    rsx! {
      div { class: "border rounded p-3 bg-dark text-light h-100 d-flex flex-column",
        // Header & Quick Add Toolbar
        div { class: "d-flex justify-content-between align-items-center mb-2 pb-2 border-bottom border-secondary flex-wrap gap-2",
          div {
            div { class: "fw-bold small d-flex align-items-center gap-2",
              Icon { icon: FaEye }
              "Merit Function Operands"
            }
            div { class: "text-muted small",
              "Intuitive criteria without 4-letter mnemonics"
            }
          }
          div { class: "btn-group btn-group-sm",
            Button {
              variant: ButtonVariant::Primary,
              size: ButtonSize::Sm,
              title: "Add Aberration / Spot criterion",
              Icon { icon: FaPlus }
              "Target Value"
            }
            Button {
              variant: ButtonVariant::Outline,
              size: ButtonSize::Sm,
              title: "Add boundary / packaging constraint",
              Icon { icon: FaPlus }
              "Constraint"
            }
          }
        }

        // Operands Cards List
        ScrollArea { height: "23rem",
          div { class: "d-flex flex-column gap-3 pe-2",
            for op in props.operands.read().iter() {
              div {
                key: "{op.id}",
                class: "card bg-black bg-opacity-25 border border-secondary text-light p-2",
                // Card Top: Title, Badge and Weight
                div { class: "d-flex justify-content-between align-items-start mb-2",
                  div {
                    div { class: "fw-bold text-white small d-flex align-items-center gap-2",
                      match op.category {
                          "Envelope" => rsx! {
                            Icon { icon: FaRulerCombined }
                          },
                          "Geometry" => rsx! {
                            Icon { icon: FaCube }
                          },
                          _ => rsx! {
                            Icon { icon: FaEye }
                          },
                      }
                      "{op.title}"
                    }
                    div { class: "text-muted small", "{op.target_description}" }
                  }
                  div { class: "d-flex align-items-center gap-2",
                    span { class: "badge bg-info text-dark font-monospace",
                      "Weight: {op.weight:.1}"
                    }
                    button {
                      class: "btn btn-outline-danger btn-sm py-0 px-1 border-0",
                      title: "Remove criterion",
                      Icon { icon: FaTrash }
                    }
                  }
                }

                // Card Middle: Current value vs Target
                div { class: "row g-2 align-items-center bg-dark p-2 rounded mx-0 mb-2 border border-secondary",
                  div { class: "col-6 small",
                    span { class: "text-muted d-block", "Current Istwert" }
                    span { class: "font-monospace fw-bold text-warning",
                      "{op.current_value}"
                    }
                  }
                  div { class: "col-6 small",
                    span { class: "text-muted d-block", "Target / Sollwert" }
                    span { class: "font-monospace fw-bold text-success",
                      "{op.target_value}"
                    }
                  }
                }

                // Card Bottom: Cost contribution progress bar
                div { class: "small d-flex justify-content-between text-muted mb-1",
                  span { "Merit Cost Impact" }
                  span { class: "font-monospace", "{op.cost_share_percent:.1}%" }
                }
                div {
                  class: "progress bg-secondary",
                  style: "height: 6px;",
                  div {
                    class: if op.cost_share_percent > 50.0 { "progress-bar bg-danger" } else { "progress-bar bg-primary" },
                    style: "width: {op.cost_share_percent}%;",
                  }
                }
              }
            }
          }
        }

        // Summary Footer
        div { class: "mt-auto pt-2 border-top border-secondary d-flex justify-content-between align-items-center small text-muted",
          span { "Combined Evaluation: 3 Active Criteria" }
          span { class: "badge bg-success d-flex align-items-center gap-1",
            Icon { icon: FaCheck }
            "All Pre-flight Checks Passed"
          }
        }
      }
    }
}