use crate::components::{
    logger::LogResultExt,
    node_editor::inputs::{InputData, InputParam, IntoInputData, IntoInputDataStrings},
};
use dioxus::prelude::*;
use opossum_core::position_distributions::{HexagonalTiling, PosDistType};
use opossum_core::{meter, utils::try_f64_to_u8};
use strum::{EnumIter, IntoEnumIterator};

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
            HexagonalTilingParam::Radius => Self::Length("Radius".into()),
            HexagonalTilingParam::CenterX => Self::Length("Center X".into()),
            HexagonalTilingParam::CenterY => Self::Length("Center Y".into()),
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
            Self::Radius => format!("{}", obj.radius().value),
            Self::CenterX => format!("{}", obj.center().x.value),
            Self::CenterY => format!("{}", obj.center().y.value),
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
            Self::NrOfHex => move |obj: &mut HexagonalTiling, val: f64| {
                if let Some(val) = try_f64_to_u8(val) {
                    obj.set_nr_of_hex_along_radius(val);
                }
            },
            Self::Radius => move |obj: &mut HexagonalTiling, val: f64| {
                obj.set_radius(meter!(val))
                    .log_err_with_context("`set_radius` of hexagonal_tiling");
            },
            Self::CenterX => move |obj: &mut HexagonalTiling, val: f64| {
                obj.set_center_x(meter!(val))
                    .log_err_with_context("`set_center_x` of hexagonal_tiling");
            },
            Self::CenterY => move |obj: &mut HexagonalTiling, val: f64| {
                obj.set_center_y(meter!(val))
                    .log_err_with_context("`set_center_y` of hexagonal_tiling");
            },
        }
    }
}

pub fn get_hexagonal_input_params(
    hexagonal: &HexagonalTiling,
    pos_dist_type_sig: Signal<PosDistType>,
) -> Vec<InputData> {
    let mut input_data = Vec::<InputData>::new();
    for enum_variant in HexagonalTilingParam::iter() {
        input_data.push(
            IntoInputData::<f64, HexagonalTiling, PosDistType>::to_input_data(
                &enum_variant,
                *hexagonal,
                pos_dist_type_sig,
            ),
        );
    }
    input_data
}
