use dioxus::prelude::*;
use opossum_core::{
    absorption::absorption_model::AbsorptionModel,
    material::OpticalProperties,
    refractive_index::RefractiveIndexType,
};

// Adjust import path according to your module layout if needed
use crate::components::{
    inputs::{RefractiveIndexEditor, absorption_editor::AbsorptionEditor}, primitives::card::{Card, CardContent, CardHeader, CardTitle},
};

/// Actions representing modifications in optical properties.
#[derive(Debug, Clone, PartialEq)]
pub enum OpticalPropertiesChangeAction {
    /// The refractive index model or its coefficients changed.
    RefractiveIndex(RefractiveIndexType),
    /// The absorption model or its parameters changed.
    Absorption(AbsorptionModel),
}

impl OpticalPropertiesChangeAction {
    /// Applies the change action directly to the given `OpticalProperties`.
    pub fn apply(self, optical: &mut opossum_core::material::OpticalProperties) {
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

/// Editor component for optical material properties.
///
/// Embeds both `RefractiveIndexEditor` and `AbsorptionEditor` to manage
/// dispersion and absorption characteristics of optical media.
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

    // 1. Memoized signals for both sub-models
    let ref_ind_memo = use_memo(move || optical.read().refractive_index.clone());
    let absorption_memo = use_memo(move || optical.read().absorption.clone());

    // 2. Event handlers forwarding changes to the parent
    let handle_ref_ind_change = use_callback(move |new_model: RefractiveIndexType| {
        on_change.call(OpticalPropertiesChangeEvent {
            action: OpticalPropertiesChangeAction::RefractiveIndex(new_model),
        });
    });

    let handle_absorption_change = use_callback(move |new_model: AbsorptionModel| {
        on_change.call(OpticalPropertiesChangeEvent {
            action: OpticalPropertiesChangeAction::Absorption(new_model),
        });
    });

    rsx! {
        Card {
            CardHeader {
                CardTitle { "Optical properties" }
            }
            CardContent {
                // Section 1: Refractive Index & Dispersion Model
                div { class: "mb-4",
                    h6 { class: "fw-bold text-secondary mb-2", "Dispersion & Refractive Index" }
                    RefractiveIndexEditor {
                        value: ref_ind_memo,
                        base_id: format!("{}_ref_ind", base_id),
                        readonly,
                        on_change: handle_ref_ind_change,
                    }
                }

                hr { class: "my-3" }

                // Section 2: Absorption Model
                div { class: "mb-2",
                    h6 { class: "fw-bold text-secondary mb-2", "Absorption & Attenuation" }
                    AbsorptionEditor {
                        value: absorption_memo,
                        base_id: format!("{}_absorption", base_id),
                        readonly,
                        on_change: handle_absorption_change,
                    }
                }
            }
        }
    }
}