use crate::components::node_editor::inputs::{
    InputData, InputParam, IntoInputData, IntoInputDataStrings,
};
use dioxus::prelude::*;
use opossum_backend::{Hexapolar, PosDistType, millimeter};
use strum::{EnumIter, IntoEnumIterator};
use uom::si::length::millimeter;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum HexapolarParam {
    NrOfRings,
    Radius,
}

impl From<HexapolarParam> for InputParam {
    fn from(value: HexapolarParam) -> Self {
        match value {
            HexapolarParam::NrOfRings => Self::U8("#Rings".into()),
            HexapolarParam::Radius => Self::Length("Radius in mm".into()),
        }
    }
}

impl IntoInputDataStrings<Hexapolar> for HexapolarParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::NrOfRings => "NrOfRings",
            Self::Radius => "Radius",
        };
        format!("hexapolar{id_str}Input")
    }

    fn create_value_string(&self, obj: &Hexapolar) -> String {
        match self {
            Self::NrOfRings => format!("{}", obj.nr_of_rings()),
            Self::Radius => format!("{:.3e}", obj.radius().get::<millimeter>()),
        }
    }
}

impl IntoInputData<u8, Hexapolar, PosDistType> for HexapolarParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<u8> {
        let e_value = e.value();
        e_value.parse::<u8>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut Hexapolar, u8) {
        match self {
            Self::NrOfRings => move |obj: &mut Hexapolar, val: u8| obj.set_nr_of_rings(val),
            Self::Radius => move |_: &mut Hexapolar, _: u8| {},
        }
    }
}

impl IntoInputData<f64, Hexapolar, PosDistType> for HexapolarParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut Hexapolar, f64) {
        match self {
            Self::NrOfRings => move |_: &mut Hexapolar, _: f64| {},
            Self::Radius => move |obj: &mut Hexapolar, val: f64| obj.set_radius(millimeter!(val)),
        }
    }
}

pub fn get_hexapolar_input_params(
    hexapolar: &Hexapolar,
    pos_dist_type_sig: Signal<PosDistType>,
) -> Vec<InputData> {
    let mut input_data = Vec::<InputData>::new();
    for enum_variant in HexapolarParam::iter() {
        match enum_variant {
            HexapolarParam::NrOfRings => {
                input_data.push(IntoInputData::<u8, Hexapolar, PosDistType>::to_input_data(
                    &enum_variant,
                    *hexapolar,
                    pos_dist_type_sig,
                ));
            }
            HexapolarParam::Radius => {
                input_data.push(IntoInputData::<f64, Hexapolar, PosDistType>::to_input_data(
                    &enum_variant,
                    *hexapolar,
                    pos_dist_type_sig,
                ));
            }
        }
    }
    input_data
}
