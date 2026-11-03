use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaChartLine, FaGears},
};

#[derive(Props, Clone, PartialEq)]
pub struct LiveMonitorProps {
    pub is_running: bool,
    pub iteration: u64,
    pub max_iterations: u64,
    pub current_cost: f64,
    pub initial_cost: f64,
}

#[component]
pub fn LiveMonitorPanel(props: LiveMonitorProps) -> Element {
    // Generate dummy SVG path points for log-cost curve
    let points = "10,25 35,50 70,85 110,105 160,118 210,122 260,124";

    rsx! {
      div { class: "border rounded p-3 bg-dark text-light h-100 d-flex flex-column",
        // Header: Solver Setup
        div { class: "fw-bold small d-flex align-items-center gap-2 mb-2 pb-2 border-bottom border-secondary",
          Icon { icon: FaGears }
          "Solver Execution"
        }

        // Algorithm settings
        div { class: "mb-3",
          label { class: "form-label small text-muted mb-1", "Numerical Algorithm" }
          select { class: "form-select form-select-sm bg-dark text-light border-secondary",
            option { selected: true, "L-BFGS (Quasi-Newton)" }
            option { "Nelder-Mead (Simplex)" }
            option { "Simulated Annealing (Global)" }
          }
        }

        div { class: "row g-2 mb-3",
          div { class: "col-6",
            label { class: "form-label small text-muted mb-1", "Max Iters" }
            input {
              r#type: "number",
              class: "form-control form-control-sm bg-dark text-light border-secondary font-monospace",
              value: "{props.max_iterations}",
            }
          }
          div { class: "col-6",
            label { class: "form-label small text-muted mb-1", "Target Cost" }
            input {
              class: "form-control form-control-sm bg-dark text-light border-secondary font-monospace",
              value: "1e-8",
            }
          }
        }

        // Live Metrics Counter
        div { class: "p-2 rounded bg-black bg-opacity-25 border border-secondary mb-3",
          div { class: "d-flex justify-content-between align-items-center small mb-1",
            span { class: "text-muted", "Progress" }
            span { class: "font-monospace text-info",
              "Iter {props.iteration} / {props.max_iterations}"
            }
          }
          div { class: "d-flex justify-content-between align-items-center small",
            span { class: "text-muted", "Merit Φ(x)" }
            span { class: "font-monospace fw-bold text-warning", "{props.current_cost:.5}" }
          }
        }

        // SVG Convergence Plot (Log-Scale Mock)
        div { class: "fw-bold small d-flex align-items-center gap-2 mb-1",
          Icon { icon: FaChartLine }
          "Convergence Log Φ"
        }
        div {
          class: "border border-secondary rounded p-1 bg-black mb-3 position-relative",
          style: "height: 140px;",
          svg { view_box: "0 0 280 130", class: "w-100 h-100",
            // Horizontal Grid Lines
            line {
              x1: "0",
              y1: "30",
              x2: "280",
              y2: "30",
              stroke: "#333",
              stroke_dasharray: "3",
            }
            line {
              x1: "0",
              y1: "70",
              x2: "280",
              y2: "70",
              stroke: "#333",
              stroke_dasharray: "3",
            }
            line {
              x1: "0",
              y1: "110",
              x2: "280",
              y2: "110",
              stroke: "#333",
              stroke_dasharray: "3",
            }

            // Convergence Curve
            polyline {
              fill: "none",
              stroke: "#00d26a",
              stroke_width: "2.5",
              points: "{points}",
            }

            // Best Point Indicator
            circle {
              cx: "260",
              cy: "124",
              r: "4",
              fill: "#00d26a",
            }
          }
          span {
            class: "position-absolute top-0 end-0 p-1 text-muted font-monospace",
            style: "font-size: 10px;",
            "-85% drop"
          }
        }

        // Live status banner
        div { class: "mt-auto",
          if props.is_running {
            div { class: "alert alert-primary py-1 px-2 mb-0 small d-flex align-items-center gap-2",
              span { class: "spinner-border spinner-border-sm" }
              "Evaluating gradients via Rayon..."
            }
          } else {
            div { class: "alert alert-secondary py-1 px-2 mb-0 small text-center text-muted",
              "Ready to optimize"
            }
          }
        }
      }
    }
}