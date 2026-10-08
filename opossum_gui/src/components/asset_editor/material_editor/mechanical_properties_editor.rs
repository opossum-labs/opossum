use dioxus::prelude::*;
use opossum_core::material::MechanicalProperties;
use uom::si::f64::{MassDensity, Pressure};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;

use super::OptionalPropertyInput;
use crate::components::primitives::card::{Card, CardContent, CardHeader, CardTitle};

#[derive(Debug, Clone, PartialEq)]
pub enum MechanicalPropertiesChangeAction {
    Density(Option<MassDensity>),
    YoungsModulus(Option<Pressure>),
}

impl MechanicalPropertiesChangeAction {
    pub fn apply(self, mechanical: &mut Option<MechanicalProperties>) {
        let mut props = mechanical.clone().unwrap_or_default();
        match self {
            Self::Density(d) => props.density = d,
            Self::YoungsModulus(y) => props.youngs_modulus = y,
        }
        // If both values are erased, remove the entire struct
        if props.density.is_none() && props.youngs_modulus.is_none() {
            *mechanical = None;
        } else {
            *mechanical = Some(props);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MechanicalPropertiesChangeEvent {
    pub action: MechanicalPropertiesChangeAction,
}

#[component]
pub fn MechanicalPropertiesEditor(
    mechanical: ReadSignal<Option<MechanicalProperties>>,
    on_change: EventHandler<MechanicalPropertiesChangeEvent>,
    #[props(default = "mechanicalProps".to_string())] base_id: String,
    #[props(default = false)] readonly: bool,
) -> Element {
    debug!("🔄 Render: MechanicalPropertiesEditor");

    // Convert from base units (kg/m^3 and Pa) to display units (g/cm^3 and GPa)
    let density_val = mechanical
        .read()
        .as_ref()
        .and_then(|m| m.density)
        .map(|d| d.get::<kilogram_per_cubic_meter>() / 1000.0);

    let youngs_val = mechanical
        .read()
        .as_ref()
        .and_then(|m| m.youngs_modulus)
        .map(|p| p.get::<pascal>() / 1e9);

    rsx! {
      Card {
        CardHeader {
          CardTitle { "Mechanical properties" }
        }
        CardContent {
          div { class: "row gy-3",
            div { class: "col-md-6",
              OptionalPropertyInput {
                id: format!("{}_density", base_id),
                label: "Density [g/cm³]".to_string(),
                value: density_val,
                readonly,
                on_change: move |v: Option<f64>| {
                    on_change
                        .call(MechanicalPropertiesChangeEvent {
                            action: MechanicalPropertiesChangeAction::Density(
                                v
                                    .map(|val| MassDensity::new::<
                                        kilogram_per_cubic_meter,
                                    >(val * 1000.0)),
                            ),
                        });
                },
              }
            }
            div { class: "col-md-6",
              OptionalPropertyInput {
                id: format!("{}_youngs", base_id),
                label: "Young's Modulus [GPa]".to_string(),
                value: youngs_val,
                readonly,
                on_change: move |v: Option<f64>| {
                    on_change
                        .call(MechanicalPropertiesChangeEvent {
                            action: MechanicalPropertiesChangeAction::YoungsModulus(
                                v.map(|val| Pressure::new::<pascal>(val * 1e9)),
                            ),
                        });
                },
              }
            }
          }
        }
      }
    }
}
