use crate::components::node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings};
use dioxus::prelude::*;
use opossum_core::meter;
use opossum_core::refractive_index::{RefrIndexConrady, RefractiveIndexType};
use strum::EnumIter;

#[derive(Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum ConradyParam {
    WaveLengthStart,
    WavelengthEnd,
    A,
    B,
    C,
}

impl From<ConradyParam> for InputParam {
    fn from(value: ConradyParam) -> Self {
        match value {
            ConradyParam::WaveLengthStart => Self::Length("Start λ".into()),
            ConradyParam::WavelengthEnd => Self::Length("End λ".into()),
            ConradyParam::A => Self::F64("A".into()),
            ConradyParam::B => Self::F64("B".into()),
            ConradyParam::C => Self::F64("C".into()),
        }
    }
}

impl IntoInputDataStrings<RefrIndexConrady> for ConradyParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::WaveLengthStart => "WvlStart",
            Self::WavelengthEnd => "WvlEnd",
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
        };

        format!("refractiveIndexConrady{id_str}Input")
    }
    fn create_value_string(&self, obj: &RefrIndexConrady) -> String {
        match self {
            Self::WaveLengthStart => {
                format!("{}", obj.wavelength_range().start.value)
            }
            Self::WavelengthEnd => format!("{}", obj.wavelength_range().end.value),
            Self::A => format!("{:.3e}", obj.n0()),
            Self::B => format!("{:.3e}", obj.a()),
            Self::C => format!("{:.3e}", obj.b()),
        }
    }
}

impl IntoInputData<f64, RefrIndexConrady, RefractiveIndexType> for ConradyParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut RefrIndexConrady, f64) {
        match self {
            Self::WaveLengthStart => move |obj: &mut RefrIndexConrady, val: f64| {
                obj.set_wavelength_range_start(meter!(val));
            },
            Self::WavelengthEnd => move |obj: &mut RefrIndexConrady, val: f64| {
                obj.set_wavelength_range_end(meter!(val));
            },
            Self::A => move |obj: &mut RefrIndexConrady, val: f64| obj.set_n0(val),
            Self::B => move |obj: &mut RefrIndexConrady, val: f64| obj.set_a(val),
            Self::C => move |obj: &mut RefrIndexConrady, val: f64| obj.set_b(val),
        }
    }
}
