use crate::components::primitives::{
    alert_dialog::{AlertDialog, AlertDialogActions, AlertDialogCancel, AlertDialogDescription},
    button::{Button, ButtonSize, ButtonVariant},
    card::{Card, CardAction, CardContent, CardHeader, CardTitle},
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaBullseye, FaPlay, FaRotateLeft, FaStop},
};

use super::{
    live_monitor_panel::LiveMonitorPanel,
    merit_designer_panel::MeritDesignerPanel,
    toolbar::OptimizationToolbar,
    variables_panel::VariablesPanel,
};

/// Mock data model for an optical free variable.
#[derive(Clone, PartialEq, Debug)]
pub struct MockVariable {
    pub id: &'static str,
    pub node_name: &'static str,
    pub property_name: &'static str,
    pub current_value: f64,
    pub unit: &'static str,
    pub min_bound: f64,
    pub max_bound: f64,
    pub is_enabled: bool,
}

/// Mock data model for a parameter coupling.
#[derive(Clone, PartialEq, Debug)]
pub struct MockCoupling {
    pub target_node: &'static str,
    pub target_prop: &'static str,
    pub rule_desc: &'static str,
    pub source_node: &'static str,
    pub source_prop: &'static str,
}

/// Mock data model for an optimization operand.
#[derive(Clone, PartialEq, Debug)]
pub struct MockOperand {
    pub id: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub target_description: &'static str,
    pub current_value: String,
    pub target_value: String,
    pub weight: f64,
    pub cost_share_percent: f64,
}

/// Main modal dialog for optical system optimization.
#[component]
pub fn OptimizationModal(open: Signal<bool>) -> Element {
    // Mock State: Free variables
    let variables = use_signal(|| vec![
        MockVariable {
            id: "var_1",
            node_name: "75 mm Lens",
            property_name: "front curvature",
            current_value: 130.0,
            unit: "mm",
            min_bound: 40.0,
            max_bound: 250.0,
            is_enabled: true,
        },
        MockVariable {
            id: "var_2",
            node_name: "Distance Lens1 -> Lens2",
            property_name: "distance",
            current_value: 125.0,
            unit: "mm",
            min_bound: 80.0,
            max_bound: 180.0,
            is_enabled: true,
        },
        MockVariable {
            id: "var_3",
            node_name: "Distance Lens2 -> Spot",
            property_name: "distance",
            current_value: 45.0,
            unit: "mm",
            min_bound: 20.0,
            max_bound: 90.0,
            is_enabled: true,
        },
    ]);

    // Mock State: Couplings
    let couplings = use_signal(|| vec![MockCoupling {
        target_node: "75 mm Lens",
        target_prop: "rear curvature",
        rule_desc: "Opposite (-1.0 ×)",
        source_node: "75 mm Lens",
        source_prop: "front curvature",
    }]);

    // Mock State: Operands (Merit function)
    let operands = use_signal(|| vec![
        MockOperand {
            id: "op_spot",
            title: "RMS Spot Radius",
            category: "Image Quality",
            target_description: "Spot Detector @ Analyzer 1 (1064 nm)",
            current_value: "38.4 µm".to_string(),
            target_value: "0.0 µm".to_string(),
            weight: 1.0,
            cost_share_percent: 78.5,
        },
        MockOperand {
            id: "op_edge",
            title: "Min Lens Edge Thickness",
            category: "Geometry",
            target_description: "75 mm Lens (min thickness >= 1.5 mm)",
            current_value: "1.82 mm".to_string(),
            target_value: ">= 1.50 mm".to_string(),
            weight: 50.0,
            cost_share_percent: 0.0,
        },
        MockOperand {
            id: "op_len",
            title: "Max Total System Length",
            category: "Envelope",
            target_description: "Lens1 -> Lens2 -> Detector",
            current_value: "170.0 mm".to_string(),
            target_value: "<= 185.0 mm".to_string(),
            weight: 20.0,
            cost_share_percent: 21.5,
        },
    ]);

    // Mock State: Optimization run & Convergence
    let mut is_running = use_signal(|| false);
    let mut current_iteration = use_signal(|| 42_u64);
    let max_iterations = use_signal(|| 100_u64);
    let mut current_cost = use_signal(|| 0.0384_f64);
    let initial_cost = 2.451_f64;

    // Simulated iteration step
    let toggle_run = move |_| {
        let running = !is_running();
        is_running.set(running);
        if running {
            // Update dummy progress
            current_iteration.set(43);
            current_cost.set(0.0120);
        }
    };

    rsx! {
      AlertDialog {
        open: open(),
        on_open_change: move |v: bool| open.set(v),
        max_width: "82rem".to_string(),
        AlertDialogDescription {
          Card {
            // Header Bar
            CardHeader {
              CardTitle {
                span { class: "d-flex align-items-center gap-2",
                  Icon { icon: FaBullseye }
                  "Optical System Optimization"
                }
              }
              CardAction {
                div { class: "d-flex align-items-center gap-2",
                  Button {
                    variant: if is_running() { ButtonVariant::Destructive } else { ButtonVariant::Primary },
                    size: ButtonSize::Sm,
                    onclick: toggle_run,
                    if is_running() {
                      Icon { icon: FaStop }
                      "Abort Optimizer"
                    } else {
                      Icon { icon: FaPlay }
                      "Run Optimization"
                    }
                  }
                  Button {
                    variant: ButtonVariant::Outline,
                    size: ButtonSize::Sm,
                    title: "Revert system to pre-optimization blueprint",
                    disabled: is_running(),
                    Icon { icon: FaRotateLeft }
                    "Undo to Baseline"
                  }
                }
              }
            }

            CardContent {
              // Top Toolbar: Presets & Templates
              OptimizationToolbar {}

              // 3-Column Layout: Variables | Merit Designer | Live Monitor
              div { class: "row g-3",
                // Column 1: Free Variables & Couplings (30% width)
                div { class: "col-lg-4 col-md-12",
                  VariablesPanel { variables, couplings }
                }

                // Column 2: Merit Function Designer (40% width)
                div { class: "col-lg-5 col-md-12",
                  MeritDesignerPanel { operands }
                }

                // Column 3: Live Convergence Monitor & Controls (30% width)
                div { class: "col-lg-3 col-md-12",
                  LiveMonitorPanel {
                    is_running: is_running(),
                    iteration: current_iteration(),
                    max_iterations: max_iterations(),
                    current_cost: current_cost(),
                    initial_cost,
                  }
                }
              }
            }
          }
        }
        AlertDialogActions {
          AlertDialogCancel { on_click: move |_| open.set(false), "Close Window" }
        }
      }
    }
}