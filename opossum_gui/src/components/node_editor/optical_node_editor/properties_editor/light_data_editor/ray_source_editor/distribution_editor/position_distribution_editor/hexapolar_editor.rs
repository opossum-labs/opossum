use crate::components::{
    logger::LogResultExt,
    node_editor::inputs::{InputData, InputParam, IntoInputData, IntoInputDataStrings},
};
use dioxus::prelude::*;
use opossum_core::distributions::position::{Hexapolar, PosDistType};
use opossum_core::{meter, utils::try_f64_to_u8};
use strum::{EnumIter, IntoEnumIterator};

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum HexapolarParam {
    NrOfRings,
    Radius,
}

impl From<HexapolarParam> for InputParam {
    fn from(value: HexapolarParam) -> Self {
        match value {
            HexapolarParam::NrOfRings => Self::U8("#Rings".into()),
            HexapolarParam::Radius => Self::SIUnit("Radius".into(), "m".into()),
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
            Self::Radius => format!("{}", obj.radius().value),
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
            Self::NrOfRings => move |obj: &mut Hexapolar, val: f64| {
                if let Some(val) = try_f64_to_u8(val) {
                    obj.set_nr_of_rings(val);
                }
            },
            Self::Radius => move |obj: &mut Hexapolar, val: f64| {
                obj.set_radius(meter!(val))
                    .log_err_with_context("`set_radius` of hexapolar");
            },
        }
    }
}

pub fn get_hexapolar_input_params(
    hexapolar: &Hexapolar,
    pos_dist_type_handler: EventHandler<PosDistType>,
    readonly: bool,
) -> Vec<InputData> {
    let mut input_data = Vec::<InputData>::new();
    for enum_variant in HexapolarParam::iter() {
        input_data.push(IntoInputData::<f64, Hexapolar, PosDistType>::to_input_data(
            &enum_variant,
            *hexapolar,
            pos_dist_type_handler,
            readonly,
        ));
    }
    input_data
}
