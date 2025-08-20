use crate::components::node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings};
use dioxus::prelude::*;
use opossum_backend::{RefractiveIndexType, nanometer, refr_index_schott::RefrIndexSchott};
use strum::EnumIter;
use uom::si::length::nanometer;

#[derive(Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum SchottParam {
    WaveLengthStart,
    WavelengthEnd,
    A,
    B,
    C,
    D,
    E,
    F,
}

impl From<SchottParam> for InputParam {
    fn from(value: SchottParam) -> Self {
        match value {
            SchottParam::WaveLengthStart => Self::Length("Start λ in nm".into()),
            SchottParam::WavelengthEnd => Self::Length("End λ in nm".into()),
            SchottParam::A => Self::F64("A".into()),
            SchottParam::B => Self::F64("B".into()),
            SchottParam::C => Self::F64("C".into()),
            SchottParam::D => Self::F64("D".into()),
            SchottParam::E => Self::F64("E".into()),
            SchottParam::F => Self::F64("F".into()),
        }
    }
}

impl IntoInputDataStrings<RefrIndexSchott> for SchottParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::WaveLengthStart => "WvlStart",
            Self::WavelengthEnd => "WvlEnd",
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
        };

        format!("refractiveIndexSchott{id_str}Input")
    }

    fn create_value_string(&self, obj: &RefrIndexSchott) -> String {
        match self {
            Self::WaveLengthStart => {
                format!("{:.3}", obj.wavelength_range().start.get::<nanometer>())
            }
            Self::WavelengthEnd => format!("{:.3}", obj.wavelength_range().end.get::<nanometer>()),
            Self::A => format!("{:.3e}", obj.a0()),
            Self::B => format!("{:.3e}", obj.a1()),
            Self::C => format!("{:.3e}", obj.a2()),
            Self::D => format!("{:.3e}", obj.a3()),
            Self::E => format!("{:.3e}", obj.a4()),
            Self::F => format!("{:.3e}", obj.a5()),
        }
    }
}

impl IntoInputData<f64, RefrIndexSchott, RefractiveIndexType> for SchottParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut RefrIndexSchott, f64) {
        match self {
            Self::WaveLengthStart => move |obj: &mut RefrIndexSchott, val: f64| {
                obj.set_wavelength_range_start(nanometer!(val));
            },
            Self::WavelengthEnd => move |obj: &mut RefrIndexSchott, val: f64| {
                obj.set_wavelength_range_end(nanometer!(val));
            },
            Self::A => move |obj: &mut RefrIndexSchott, val: f64| obj.set_a0(val),
            Self::B => move |obj: &mut RefrIndexSchott, val: f64| obj.set_a1(val),
            Self::C => move |obj: &mut RefrIndexSchott, val: f64| obj.set_a2(val),
            Self::D => move |obj: &mut RefrIndexSchott, val: f64| obj.set_a3(val),
            Self::E => move |obj: &mut RefrIndexSchott, val: f64| obj.set_a4(val),
            Self::F => move |obj: &mut RefrIndexSchott, val: f64| obj.set_a5(val),
        }
    }
}
