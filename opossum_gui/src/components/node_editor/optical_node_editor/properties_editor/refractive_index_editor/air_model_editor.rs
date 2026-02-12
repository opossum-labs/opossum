use crate::components::{
    logger::LogResultExt,
    node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings},
};
use dioxus::{core::Event, html::FormData};
use opossum_core::refractive_index::{RefrIndexAir, RefractiveIndexType};
use opossum_core::{degree_celsius, bar};
use strum::EnumIter;
use uom::si::{pressure::bar, thermodynamic_temperature::degree_celsius};

#[derive(Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum AirParam {
    Temperature,
    Pressure,
    Humidity,
}

impl From<AirParam> for InputParam {
    fn from(value: AirParam) -> Self {
        match value {
            AirParam::Temperature => Self::SIUnit("Temperature".into(), "°C".into()),
            AirParam::Pressure => Self::SIUnit("Pressure".into(), "bar".into()),
            AirParam::Humidity => Self::F64("rel. Humidity".into()),
        }
    }
}

impl IntoInputDataStrings<RefrIndexAir> for AirParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::Temperature => "Temperature",
            Self::Pressure => "Pressure",
            Self::Humidity => "Humidity",
        };

        format!("refractiveIndexAir{id_str}Input")
    }
    fn create_value_string(&self, obj: &RefrIndexAir) -> String {
        match self {
            Self::Temperature => format!("{}", obj.temperature().get::<degree_celsius>()),
            Self::Pressure => format!("{}", obj.pressure().get::<bar>()),
            Self::Humidity => format!("{}", obj.humidity()/100.),
        }
    }
}

impl IntoInputData<f64, RefrIndexAir, RefractiveIndexType> for AirParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut RefrIndexAir, f64) {
        match self {
            Self::Temperature => move |obj: &mut RefrIndexAir, val: f64| {
                obj.set_temperature(degree_celsius!(val))
                    .log_err_with_context("validation failed in `set_temperature` of RefrIndexAir");
            },
            Self::Pressure => move |obj: &mut RefrIndexAir, val: f64| {
                obj.set_pressure(bar!(val))
                    .log_err_with_context("validation failed in `set_pressure` of RefrIndexAir");
            },
            Self::Humidity => move |obj: &mut RefrIndexAir, val: f64| {
                obj.set_humidity(val*100.)
                    .log_err_with_context("validation failed in `set_humidity` of RefrIndexAir");
            },
        }
    }
}
