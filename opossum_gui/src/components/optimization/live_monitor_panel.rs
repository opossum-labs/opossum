use dioxus::prelude::*;
use dioxus_free_icons::{Icon, icons::fa_solid_icons::FaGears};
use super::state::OptimizationModel;

#[derive(Props, Clone, PartialEq)]
pub struct LiveMonitorProps {
    pub model: Signal<OptimizationModel>,
    pub is_running: bool,
    pub iteration: u64,
    pub current_cost: f64,
    pub initial_cost: f64,
    pub steps_without_improvement: u64,
}

#[component]
pub fn LiveMonitorPanel(mut props: LiveMonitorProps) -> Element {
    let current_algo_kind = props.model.read().algorithm_kind();
    let configured_max_iters = props.model.read().max_iterations();

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

        // Combined Solver Hyperparameters Box (Iterations & optional SA Parameters)
        div { class: "p-2 rounded bg-black bg-opacity-50 border border-secondary mb-2",
          div { class: "row g-2 align-items-center",
            div { class: "col-12",
              label {
                class: "form-label text-muted mb-0",
                style: "font-size: 10px;",
                "Max Iterations"
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

            // Optional Annealing Parameters embedded within the same container
            if current_algo_kind == "SimulatedAnnealing" {
              div { class: "col-6 mt-2 pt-2 border-top border-secondary border-opacity-50",
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
              div { class: "col-6 mt-2 pt-2 border-top border-secondary border-opacity-50",
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
        }

        // Real-Time Iteration & Cost Display (Target Phi removed)
        div { class: "p-2 rounded bg-black border border-secondary mb-2 font-monospace",
          div { class: "d-flex justify-content-between align-items-center small",
            span { class: "text-muted", "Step:" }
            span { class: "text-info fw-bold", "{props.iteration} / {configured_max_iters}" }
          }
          div { class: "d-flex justify-content-between align-items-center small mt-1",
            span { class: "text-muted", "Merit Φ:" }
            span { class: "text-warning fw-bold", "{props.current_cost:.6}" }
          }
        }

        // Convergence & Stagnation Health Box
        div { class: "p-2 rounded bg-black border border-secondary mb-2",
          div { class: "d-flex justify-content-between align-items-center small mb-1",
            span { class: "text-muted", "Stagnant Steps:" }
            span { class: if props.steps_without_improvement > 20 { "text-warning fw-bold font-monospace" } else if props.steps_without_improvement > 50 { "text-danger fw-bold font-monospace" } else { "text-light font-monospace" },
              "{props.steps_without_improvement}"
            }
          }
          // Relative improvement since start
          {
              let initial = props.initial_cost;
              let current = props.current_cost;
              let ratio = if initial > 0.0 {
                  ((initial - current) / initial * 100.0).clamp(-100.0, 100.0)
              } else {
                  0.0
              };
              rsx! {
                div { class: "d-flex justify-content-between align-items-center small mb-1",
                  span { class: "text-muted", "Improvement:" }
                  span { class: "text-success fw-bold font-monospace", "{ratio:.1}%" }
                }
                div { class: "progress bg-secondary", style: "height: 4px;",
                  div {
                    class: "progress-bar bg-success",
                    style: "width: {ratio.max(0.0):.1}%;",
                  }
                }
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