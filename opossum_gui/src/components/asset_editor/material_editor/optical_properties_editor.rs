use dioxus::prelude::*;
use opossum_core::{material::OpticalProperties, refractive_index::RefractiveIndexType};

// Adjust import path according to your module layout
use crate::components::{
    inputs::RefractiveIndexEditor,
    primitives::card::{Card, CardContent, CardHeader, CardTitle},
};

/// Actions representing modifications in optical properties.
#[derive(Debug, Clone, PartialEq)]
pub enum OpticalPropertiesChangeAction {
    /// The refractive index model or its coefficients changed.
    RefractiveIndex(RefractiveIndexType),
    /// The absorption coefficient changed (None if cleared).
    Absorption(Option<f64>),
}

impl OpticalPropertiesChangeAction {
    /// Applies the change action directly to the given `OpticalProperties`.
    pub const fn apply(self, optical: &mut opossum_core::material::OpticalProperties) {
        match self {
            Self::RefractiveIndex(new_model) => optical.refractive_index = new_model,
            Self::Absorption(abs) => optical.absorption = abs,
        }
    }
}

/// Event emitted when any optical property is modified by the user.
#[derive(Debug, Clone, PartialEq)]
pub struct OpticalPropertiesChangeEvent {
    /// The specific modification action.
    pub action: OpticalPropertiesChangeAction,
}

/// Helper function to parse an optional floating-point number from a string.
/// Returns `Some(None)` if the input is empty (representing a cleared field).
/// Returns `Some(Some(value))` if the input was successfully parsed into an f64.
/// Returns `None` if the parsing fails (e.g., due to invalid characters).
#[allow(clippy::option_option)]
fn parse_optional_f64(val: &str) -> Option<Option<f64>> {
    let trimmed = val.trim();
    if trimmed.is_empty() {
        Some(None)
    } else {
        trimmed.parse::<f64>().ok().map(Some)
    }
}

/// Editor component for optical material properties.
///
/// Embeds the `RefractiveIndexEditor` and provides inputs for optional absorption.
#[component]
pub fn OpticalPropertiesEditor(
    /// Read-only signal containing the current optical properties.
    optical: ReadSignal<OpticalProperties>,

    /// Event handler triggered when optical properties are changed.
    on_change: EventHandler<OpticalPropertiesChangeEvent>,

    /// Base ID used for HTML element IDs to avoid DOM collisions.
    #[props(default = "opticalProps".to_string())]
    base_id: String,

    /// If true, disables all input fields and dropdowns.
    #[props(default = false)]
    readonly: bool,
) -> Element {
    info!("🔄 Render: OpticalPropertiesEditor");

    // Derive a memoized read-signal for the refractive index model
    let ref_ind_memo = use_memo(move || optical.read().refractive_index.clone());

    // Format current absorption value for display
    let absorption_str = optical
        .read()
        .absorption
        .map_or_else(String::new, |val| format!("{val}"));

    let handle_ref_ind_change = use_callback(move |new_model: RefractiveIndexType| {
        on_change.call(OpticalPropertiesChangeEvent {
            action: OpticalPropertiesChangeAction::RefractiveIndex(new_model),
        });
    });

    let handle_absorption_change = use_callback(move |e: Event<FormData>| {
        if let Some(opt_absorption) = parse_optional_f64(&e.value()) {
            on_change.call(OpticalPropertiesChangeEvent {
                action: OpticalPropertiesChangeAction::Absorption(opt_absorption),
            });
        }
    });

    rsx! {
        Card {
            CardHeader {
                CardTitle { "Optical properties" }
            }
            CardContent {
                // Section 1: Embedded Refractive Index Editor
                div { class: "mb-4",
                    h6 { class: "fw-bold text-secondary mb-2", "Dispersion & Refractive Index" }
                    RefractiveIndexEditor {
                        value: ref_ind_memo,
                        base_id: format!("{}_ref_ind", base_id),
                        readonly,
                        on_change: handle_ref_ind_change,
                    }
                }
                hr {}
                // Section 2: Absorption Coefficient
                div { class: "row",
                    div { class: "col-md-6",
                        label {
                            class: "form-label fw-bold",
                            r#for: format!("{}_absorption", base_id),
                            "Absorption Coefficient (1/m)"
                        }
                        input {
                            id: format!("{}_absorption", base_id),
                            class: "form-control",
                            r#type: "number",
                            step: "any",
                            min: "0",
                            placeholder: "e.g., 0.001 (optional)",
                            value: "{absorption_str}",
                            readonly,
                            oninput: handle_absorption_change,
                        }
                        div { class: "form-text text-muted small",
                            "Leave empty if linear absorption is neglected."
                        }
                    }
                }
            }
        }
    }
}
