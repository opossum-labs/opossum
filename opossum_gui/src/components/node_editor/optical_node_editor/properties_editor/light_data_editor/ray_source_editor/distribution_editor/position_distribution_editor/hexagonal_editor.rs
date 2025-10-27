use crate::components::node_editor::inputs::{
    InputData, InputParam, IntoInputData, IntoInputDataStrings,
};
use dioxus::prelude::*;
use opossum_core::millimeter;
use opossum_core::position_distributions::{HexagonalTiling, PosDistType};
use strum::{EnumIter, IntoEnumIterator};
use uom::si::length::millimeter;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum HexagonalTilingParam {
    NrOfHex,
    Radius,
    CenterX,
    CenterY,
}

impl From<HexagonalTilingParam> for InputParam {
    fn from(value: HexagonalTilingParam) -> Self {
        match value {
            HexagonalTilingParam::NrOfHex => Self::U8("#Hexagons".into()),
            HexagonalTilingParam::Radius => Self::Length("Radius in mm".into()),
            HexagonalTilingParam::CenterX => Self::Length("Center X in mm".into()),
            HexagonalTilingParam::CenterY => Self::Length("Center Y in mm".into()),
        }
    }
}

impl IntoInputDataStrings<HexagonalTiling> for HexagonalTilingParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::NrOfHex => "NrOfHex",
            Self::Radius => "Radius",
            Self::CenterX => "CenterX",
            Self::CenterY => "CenterY",
        };
        format!("hexagonalTiling{id_str}Input")
    }

    fn create_value_string(&self, obj: &HexagonalTiling) -> String {
        match self {
            Self::NrOfHex => format!("{}", obj.nr_of_hex_along_radius()),
            Self::Radius => format!("{:.3e}", obj.radius().get::<millimeter>()),
            Self::CenterX => format!("{:.3e}", obj.center().x.get::<millimeter>()),
            Self::CenterY => format!("{:.3e}", obj.center().y.get::<millimeter>()),
        }
    }
}

impl IntoInputData<f64, HexagonalTiling, PosDistType> for HexagonalTilingParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut HexagonalTiling, f64) {
        match self {
            Self::NrOfHex => move |_: &mut HexagonalTiling, _: f64| {},
            Self::Radius => {
                move |obj: &mut HexagonalTiling, val: f64| obj.set_radius(millimeter!(val))
            }
            Self::CenterX => {
                move |obj: &mut HexagonalTiling, val: f64| obj.set_center_x(millimeter!(val))
            }
            Self::CenterY => {
                move |obj: &mut HexagonalTiling, val: f64| obj.set_center_y(millimeter!(val))
            }
        }
    }
}

impl IntoInputData<u8, HexagonalTiling, PosDistType> for HexagonalTilingParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<u8> {
        let e_value = e.value();
        e_value.parse::<u8>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut HexagonalTiling, u8) {
        if self == &Self::NrOfHex {
            move |obj: &mut HexagonalTiling, val: u8| obj.set_nr_of_hex_along_radius(val)
        } else {
            move |_: &mut HexagonalTiling, _: u8| {}
        }
    }
}

pub fn get_hexagonal_input_params(
    hexagonal: &HexagonalTiling,
    pos_dist_type_sig: Signal<PosDistType>,
) -> Vec<InputData> {
    let mut input_data = Vec::<InputData>::new();
    for enum_variant in HexagonalTilingParam::iter() {
        match enum_variant {
            HexagonalTilingParam::NrOfHex => {
                input_data.push(
                    IntoInputData::<u8, HexagonalTiling, PosDistType>::to_input_data(
                        &enum_variant,
                        *hexagonal,
                        pos_dist_type_sig,
                    ),
                );
            }
            _ => input_data.push(
                IntoInputData::<f64, HexagonalTiling, PosDistType>::to_input_data(
                    &enum_variant,
                    *hexagonal,
                    pos_dist_type_sig,
                ),
            ),
        }
    }
    input_data
}
