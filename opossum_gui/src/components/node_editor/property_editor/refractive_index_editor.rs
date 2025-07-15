use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{
    nanometer, refr_index_schott::RefrIndexSchott, DefaultFromName, Proptype, RefrIndexConrady,
    RefrIndexConst, RefrIndexSellmeier1, RefractiveIndexType,
};
use strum::EnumIter;
use uom::si::length::nanometer;

use crate::components::node_editor::inputs::{
    input_components::{LabeledSelect, RowedInputs},
    select_options_from_enum_iterator, InputData, InputParam, IntoInputData, IntoInputDataStrings,
};

#[component]
pub fn RefractiveIndexEditor(
    property_key: String,
    prop_type_sig: Signal<Proptype>,
    ref_ind_sig: Signal<RefractiveIndexType>,
) -> Element {
    use_effect(move || {
        prop_type_sig.set(ref_ind_sig.read().clone().into());
    });

    let select_id = format!("refractiveIndexProperty{property_key}").to_camel_case();
    rsx! {
        LabeledSelect {
            id: select_id,
            label: "Refractive index definition",
            options: select_options_from_enum_iterator(
                &*ref_ind_sig.read(),
                None,
            ),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(ref_ind_type) = RefractiveIndexType::default_from_name(
                    val.as_str(),
                ) {
                    ref_ind_sig.set(ref_ind_type);
                }
            },
        }
        div { class: "accordion-content-wrapper-div border-start",
            RowedInputs { inputs: get_refractive_index_input_data(&ref_ind_sig.read(), ref_ind_sig) }
        }
    }
}

fn get_refractive_index_input_data(
    ref_ind_type: &RefractiveIndexType,
    ref_ind_sig: Signal<RefractiveIndexType>,
) -> Vec<InputData> {
    match ref_ind_type {
        RefractiveIndexType::Const(ref_ind) => {
            ConstRefParam::to_input_data_vec(ref_ind, ref_ind_sig)
        }
        RefractiveIndexType::Sellmeier1(ref_ind) => {
            Sellmeier1Param::to_input_data_vec(ref_ind, ref_ind_sig)
        }
        RefractiveIndexType::Schott(ref_ind) => {
            SchottParam::to_input_data_vec(ref_ind, ref_ind_sig)
        }
        RefractiveIndexType::Conrady(ref_ind) => {
            ConradyParam::to_input_data_vec(ref_ind, ref_ind_sig)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, EnumIter)]
enum ConstRefParam {
    RefractiveIndex,
}

impl From<ConstRefParam> for InputParam {
    fn from(_: ConstRefParam) -> Self {
        Self::F64("Refractive Index")
    }
}

impl IntoInputDataStrings<RefrIndexConst> for ConstRefParam {
    fn create_id_string(&self) -> String {
        "refractiveIndexConstInput".to_string()
    }
    fn create_value_string(&self, obj: &RefrIndexConst) -> String {
        format!("{:.3e}", obj.refractive_index())
    }
}

impl IntoInputData<f64, RefrIndexConst, RefractiveIndexType> for ConstRefParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut RefrIndexConst, f64) {
        move |obj: &mut RefrIndexConst, val: f64| obj.set_refractive_index(val)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, EnumIter)]
enum ConradyParam {
    WaveLengthStart,
    WavelengthEnd,
    A,
    B,
    C,
}

impl From<ConradyParam> for InputParam {
    fn from(value: ConradyParam) -> Self {
        match value {
            ConradyParam::WaveLengthStart => Self::Length("Start λ in nm"),
            ConradyParam::WavelengthEnd => Self::Length("End λ in nm"),
            ConradyParam::A => Self::F64("A"),
            ConradyParam::B => Self::F64("B"),
            ConradyParam::C => Self::F64("C"),
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
                format!("{:.3e}", obj.wavelength_range().start.get::<nanometer>())
            }
            Self::WavelengthEnd => format!("{:.3e}", obj.wavelength_range().end.get::<nanometer>()),
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
                obj.set_wavelength_range_start(nanometer!(val));
            },
            Self::WavelengthEnd => move |obj: &mut RefrIndexConrady, val: f64| {
                obj.set_wavelength_range_end(nanometer!(val));
            },
            Self::A => move |obj: &mut RefrIndexConrady, val: f64| obj.set_n0(val),
            Self::B => move |obj: &mut RefrIndexConrady, val: f64| obj.set_a(val),
            Self::C => move |obj: &mut RefrIndexConrady, val: f64| obj.set_b(val),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, EnumIter)]
enum SchottParam {
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
            SchottParam::WaveLengthStart => Self::Length("Start λ in nm"),
            SchottParam::WavelengthEnd => Self::Length("End λ in nm"),
            SchottParam::A => Self::F64("A"),
            SchottParam::B => Self::F64("B"),
            SchottParam::C => Self::F64("C"),
            SchottParam::D => Self::F64("D"),
            SchottParam::E => Self::F64("E"),
            SchottParam::F => Self::F64("F"),
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
                format!("{:.3e}", obj.wavelength_range().start.get::<nanometer>())
            }
            Self::WavelengthEnd => format!("{:.3e}", obj.wavelength_range().end.get::<nanometer>()),
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

#[derive(Clone, Copy, PartialEq, Eq, EnumIter)]
enum Sellmeier1Param {
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
