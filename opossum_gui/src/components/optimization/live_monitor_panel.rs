use crate::components::primitives::button::{Button, ButtonSize, ButtonVariant};
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
    // Smooth sample points for logarithmic convergence curve
    let points = "5,15 25,32 50,68 85,92 130,105 180,114 235,119 270,121";

    rsx! {
      div { class: "border rounded p-2 bg-dark text-light h-100 d-flex flex-column",
        div { class: "fw-bold small d-flex align-items-center gap-1 mb-2 pb-2 border-bottom border-secondary",
          Icon { icon: FaGears }
          "Solver & Progress"
        }

        // Algorithm Settings
        div { class: "mb-2",
          label {
            class: "form-label text-muted mb-0",
            style: "font-size: 10px;",
            "Optimizer Engine"
          }
          select {
            class: "form-select form-select-sm bg-black text-light border-secondary",
            style: "font-size: 11px;",
            option { selected: true, "L-BFGS (Quasi-Newton)" }
            option { "Nelder-Mead (Simplex)" }
            option { "Simulated Annealing" }
          }
        }

        div { class: "row g-2 mb-2",
          div { class: "col-6",
            label {
              class: "form-label text-muted mb-0",
              style: "font-size: 10px;",
              "Max Iters"
            }
            input {
              r#type: "number",
              class: "form-control form-control-sm bg-black text-light border-secondary font-monospace text-end",
              style: "font-size: 11px;",
              value: "{props.max_iterations}",
            }
          }
          div { class: "col-6",
            label {
              class: "form-label text-muted mb-0",
              style: "font-size: 10px;",
              "Target Cost"
            }
            input {
              class: "form-control form-control-sm bg-black text-light border-secondary font-monospace text-end",
              style: "font-size: 11px;",
              value: "1e-8",
            }
          }
        }

        // Real-Time Iteration Box
        div { class: "p-2 rounded bg-black border border-secondary mb-2 font-monospace",
          div { class: "d-flex justify-content-between align-items-center small",
            span { class: "text-muted", "Step:" }
            span { class: "text-info fw-bold", "{props.iteration} / {props.max_iterations}" }
          }
          div { class: "d-flex justify-content-between align-items-center small mt-1",
            span { class: "text-muted", "Merit Φ:" }
            span { class: "text-warning fw-bold", "{props.current_cost:.6}" }
          }
          div { class: "d-flex justify-content-between align-items-center small mt-1",
            span { class: "text-muted", "Δ Improvement:" }
            span { class: "text-success", "-88.4%" }
          }
        }

        // Logarithmic Convergence Plot (SVG)
        div { class: "fw-bold small d-flex align-items-center gap-1 mb-1",
          Icon { icon: FaChartLine }
          "Log Φ Convergence"
        }
        div {
          class: "border border-secondary rounded p-1 bg-black mb-2 position-relative flex-grow-1",
          style: "min-height: 120px;",
          svg { view_box: "0 0 280 130", class: "w-100 h-100",
            // Horizontal Reference Lines
            line {
              x1: "0",
              y1: "25",
              x2: "280",
              y2: "25",
              stroke: "#2a2a2a",
              stroke_dasharray: "2",
            }
            line {
              x1: "0",
              y1: "65",
              x2: "280",
              y2: "65",
              stroke: "#2a2a2a",
              stroke_dasharray: "2",
            }
            line {
              x1: "0",
              y1: "105",
              x2: "280",
              y2: "105",
              stroke: "#2a2a2a",
              stroke_dasharray: "2",
            }

            // Path
            polyline {
              fill: "none",
              stroke: "#00d26a",
              stroke_width: "2.0",
              points: "{points}",
            }
            circle {
              cx: "270",
              cy: "121",
              r: "3.5",
              fill: "#00d26a",
            }
          }
        }

        // Progress Banner
        div { class: "mt-auto",
          if props.is_running {
            div {
              class: "alert alert-primary py-1 px-2 mb-0 small d-flex align-items-center gap-2 font-monospace",
              style: "font-size: 11px;",
              span { class: "spinner-border spinner-border-sm" }
              "Rayon Finite Differences..."
            }
          } else {
            div {
              class: "alert alert-secondary py-1 px-2 mb-0 small text-center text-muted",
              style: "font-size: 11px;",
              "Ready to start"
            }
          }
        }
      }
    }
}