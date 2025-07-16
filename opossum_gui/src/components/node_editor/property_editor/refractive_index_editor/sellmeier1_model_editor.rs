use opossum_backend::{nanometer, RefrIndexSellmeier1, RefractiveIndexType};
use dioxus::prelude::*;
use strum::EnumIter;
use uom::si::length::nanometer;
use crate::components::node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings};



#[derive(Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum Sellmeier1Param {
    WaveLengthStart,
    WavelengthEnd,
    K1,
    K2,
    K3,
    L1,
    L2,
    L3,
}

impl From<Sellmeier1Param> for InputParam {
    fn from(value: Sellmeier1Param) -> Self {
        match value {
            Sellmeier1Param::WaveLengthStart => Self::Length("Start λ in nm"),
            Sellmeier1Param::WavelengthEnd => Self::Length("End λ in nm"),
            Sellmeier1Param::K1 => Self::F64("K1"),
            Sellmeier1Param::K2 => Self::F64("K2"),
            Sellmeier1Param::K3 => Self::F64("K3"),
            Sellmeier1Param::L1 => Self::F64("L1"),
            Sellmeier1Param::L2 => Self::F64("L2"),
            Sellmeier1Param::L3 => Self::F64("L3"),
        }
    }
}

impl IntoInputDataStrings<RefrIndexSellmeier1> for Sellmeier1Param {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::WaveLengthStart => "WvlStart",
            Self::WavelengthEnd => "WvlEnd",
            Self::K1 => "K1",
            Self::K2 => "K2",
            Self::K3 => "K3",
            Self::L1 => "L1",
            Self::L2 => "L2",
            Self::L3 => "L3",
        };

        format!("refractiveIndexSellmeier1{id_str}Input")
    }
    fn create_value_string(&self, obj: &RefrIndexSellmeier1) -> String {
        match self {
            Self::WaveLengthStart => {
                format!("{:.3e}", obj.wavelength_range().start.get::<nanometer>())
            }
            Self::WavelengthEnd => format!("{:.3e}", obj.wavelength_range().end.get::<nanometer>()),
            Self::K1 => format!("{:.3e}", obj.k1()),
            Self::K2 => format!("{:.3e}", obj.k2()),
            Self::K3 => format!("{:.3e}", obj.k3()),
            Self::L1 => format!("{:.3e}", obj.l1()),
            Self::L2 => format!("{:.3e}", obj.l2()),
            Self::L3 => format!("{:.3e}", obj.l3()),
        }
    }
}

impl IntoInputData<f64, RefrIndexSellmeier1, RefractiveIndexType> for Sellmeier1Param {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut RefrIndexSellmeier1, f64) {
        match self {
            Self::WaveLengthStart => move |obj: &mut RefrIndexSellmeier1, val: f64| {
                obj.set_wavelength_range_start(nanometer!(val));
            },
            Self::WavelengthEnd => move |obj: &mut RefrIndexSellmeier1, val: f64| {
                obj.set_wavelength_range_end(nanometer!(val));
            },
            Self::K1 => move |obj: &mut RefrIndexSellmeier1, val: f64| obj.set_k1(val),
            Self::K2 => move |obj: &mut RefrIndexSellmeier1, val: f64| obj.set_k2(val),
            Self::K3 => move |obj: &mut RefrIndexSellmeier1, val: f64| obj.set_k3(val),
            Self::L1 => move |obj: &mut RefrIndexSellmeier1, val: f64| obj.set_l1(val),
            Self::L2 => move |obj: &mut RefrIndexSellmeier1, val: f64| obj.set_l2(val),
            Self::L3 => move |obj: &mut RefrIndexSellmeier1, val: f64| obj.set_l3(val),
        }
    }
}
