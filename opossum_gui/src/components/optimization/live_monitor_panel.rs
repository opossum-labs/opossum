use crate::components::primitives::button::{Button, ButtonSize, ButtonVariant};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaChartLine, FaGears},
};

use super::state::OptimizationModel;

#[derive(Props, Clone, PartialEq)]
pub struct LiveMonitorProps {
    pub model: Signal<OptimizationModel>,
    pub is_running: bool,
    pub iteration: u64,
    pub current_cost: f64,
    pub initial_cost: f64,
}

#[component]
pub fn LiveMonitorPanel(mut props: LiveMonitorProps) -> Element {
    let current_algo_kind = props.model.read().algorithm_kind();
    let configured_max_iters = props.model.read().max_iterations();
    let configured_target_cost = props.model.read().target_cost;

    // Smooth sample points for logarithmic convergence curve placeholder
    let points = "5,15 25,32 50,68 85,92 130,105 180,114 235,119 270,121";

    rsx! {
      div { class: "border rounded p-2 bg-dark text-light h-100 d-flex flex-column",
        div { class: "fw-bold small d-flex align-items-center gap-1 mb-2 pb-2 border-bottom border-secondary",
          Icon { icon: FaGears }
          "Solver & Progress"
        }

        // Algorithm Selector
        div { class: "mb-2",
          label {
            class: "form-label text-muted mb-0",
            style: "font-size: 10px;",
            "Optimizer Engine"
          }
          select {
            class: "form-select form-select-sm bg-black text-light border-secondary",
            style: "font-size: 11px;",
            disabled: props.is_running,
            value: "{current_algo_kind}",
            onchange: move |e| {
                props.model.write().set_algorithm_kind(&e.value());
            },
            option { value: "DLS", "Damped Least Squares (Levenberg-Marquardt)" }
            option { value: "LBFGS", "L-BFGS (Quasi-Newton)" }
            option { value: "NelderMead", "Nelder-Mead (Simplex)" }
            option { value: "SimulatedAnnealing", "Simulated Annealing" }
          }
        }

        // Max Iterations & Target Cost Inputs
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
              disabled: props.is_running,
              value: "{configured_max_iters}",
              oninput: move |e| {
                  if let Ok(val) = e.value().parse::<u64>() {
                      props.model.write().set_max_iterations(val);
                  }
              },
            }
          }
          div { class: "col-6",
            label {
              class: "form-label text-muted mb-0",
              style: "font-size: 10px;",
              "Target Cost"
            }
            input {
              r#type: "number",
              step: "any",
              class: "form-control form-control-sm bg-black text-light border-secondary font-monospace text-end",
              style: "font-size: 11px;",
              disabled: props.is_running,
              value: "{configured_target_cost}",
              oninput: move |e| {
                  if let Ok(val) = e.value().parse::<f64>() {
                      props.model.write().set_target_cost(val);
                  }
              },
            }
          }
        }
// Algorithm-Specific Controls for Simulated Annealing
        if current_algo_kind == "SimulatedAnnealing" {
          div { class: "row g-2 mb-2 p-2 rounded bg-black bg-opacity-50 border border-secondary",
            div { class: "col-6",
              label {
                class: "form-label text-warning mb-0",
                style: "font-size: 10px;",
                "Initial Temp (T₀)"
              }
              input {
                r#type: "number",
                step: "0.1",
                class: "form-control form-control-sm bg-black text-light border-secondary font-monospace text-end",
                style: "font-size: 11px;",
                disabled: props.is_running,
                value: "{props.model.read().sa_initial_temperature()}",
                oninput: move |e| {
                    if let Ok(val) = e.value().parse::<f64>() {
                        props.model.write().set_sa_initial_temperature(val);
                    }
                },
              }
            }
            div { class: "col-6",
              label {
                class: "form-label text-warning mb-0",
                style: "font-size: 10px;",
                "Cooling Rate (α)"
              }
              input {
                r#type: "number",
                step: "0.01",
                min: "0.50",
                max: "0.999",
                class: "form-control form-control-sm bg-black text-light border-secondary font-monospace text-end",
                style: "font-size: 11px;",
                disabled: props.is_running,
                value: "{props.model.read().sa_cooling_rate()}",
                oninput: move |e| {
                    if let Ok(val) = e.value().parse::<f64>() {
                        props.model.write().set_sa_cooling_rate(val);
                    }
                },
              }
            }
          }
        }
        // Real-Time Iteration & Cost Display
        div { class: "p-2 rounded bg-black border border-secondary mb-2 font-monospace",
          div { class: "d-flex justify-content-between align-items-center small",
            span { class: "text-muted", "Step:" }
            span { class: "text-info fw-bold", "{props.iteration} / {configured_max_iters}" }
          }
          div { class: "d-flex justify-content-between align-items-center small mt-1",
            span { class: "text-muted", "Merit Φ:" }
            span { class: "text-warning fw-bold", "{props.current_cost:.6}" }
          }
          div { class: "d-flex justify-content-between align-items-center small mt-1",
            span { class: "text-muted", "Target Φ:" }
            span { class: "text-success", "{configured_target_cost:.1e}" }
          }
        }

        // Logarithmic Convergence Plot (SVG Placeholder)
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
              "Optimizing ({current_algo_kind})..."
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