use dioxus::prelude::*;
use opossum_core::material::ThermalProperties;
use uom::si::f64::{TemperatureCoefficient, ThermalConductivity};
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;

use super::OptionalPropertyInput;
use crate::components::primitives::card::{Card, CardContent, CardHeader, CardTitle};

#[derive(Debug, Clone, PartialEq)]
pub enum ThermalPropertiesChangeAction {
    Conductivity(Option<ThermalConductivity>),
    Expansion(Option<TemperatureCoefficient>),
}

impl ThermalPropertiesChangeAction {
    pub fn apply(self, thermal: &mut Option<ThermalProperties>) {
        let mut props = thermal.clone().unwrap_or_default();
        match self {
            Self::Conductivity(c) => props.thermal_conductivity = c,
            Self::Expansion(e) => props.expansion_coefficient = e,
        }
        // If both values are erased, remove the entire struct to keep the RON clean
        if props.thermal_conductivity.is_none() && props.expansion_coefficient.is_none() {
            *thermal = None;
        } else {
            *thermal = Some(props);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThermalPropertiesChangeEvent {
    pub action: ThermalPropertiesChangeAction,
}

#[component]
pub fn ThermalPropertiesEditor(
    thermal: ReadSignal<Option<ThermalProperties>>,
    on_change: EventHandler<ThermalPropertiesChangeEvent>,
    #[props(default = "thermalProps".to_string())] base_id: String,
    #[props(default = false)] readonly: bool,
) -> Element {
    debug!("🔄 Render: ThermalPropertiesEditor");

    // Convert from base units to display units
    let t_cond = thermal
        .read()
        .as_ref()
        .and_then(|t| t.thermal_conductivity)
        .map(|c| c.get::<watt_per_meter_kelvin>());

    let t_exp = thermal
        .read()
        .as_ref()
        .and_then(|t| t.expansion_coefficient)
        .map(|e| e.get::<per_kelvin>() * 1e6);

    rsx! {
      Card {
        CardHeader {
          CardTitle { "Thermal properties" }
        }
        CardContent {
          div { class: "row gy-3",
            div { class: "col-md-6",
              OptionalPropertyInput {
                id: format!("{}_conductivity", base_id),
                label: "Heat Conductivity [W/(m K)]".to_string(),
                value: t_cond,
                readonly,
                on_change: move |v: Option<f64>| {
                    on_change
                        .call(ThermalPropertiesChangeEvent {
                            action: ThermalPropertiesChangeAction::Conductivity(
                                v.map(ThermalConductivity::new::<watt_per_meter_kelvin>),
                            ),
                        });
                },
              }
            }
            div { class: "col-md-6",
              OptionalPropertyInput {
                id: format!("{}_expansion", base_id),
                label: "Thermal Expansion [1E-6 / K]".to_string(),
                value: t_exp,
                readonly,
                on_change: move |v: Option<f64>| {
                    on_change
                        .call(ThermalPropertiesChangeEvent {
                            action: ThermalPropertiesChangeAction::Expansion(
                                v.map(|val| TemperatureCoefficient::new::<per_kelvin>(val * 1e-6)),
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
