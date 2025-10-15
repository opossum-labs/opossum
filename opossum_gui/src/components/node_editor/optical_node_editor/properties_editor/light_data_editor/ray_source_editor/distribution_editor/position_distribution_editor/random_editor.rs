use crate::components::node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings};
use dioxus::prelude::*;
use opossum_backend::{PosDistType, Random, millimeter, try_f64_to_usize};
use strum::EnumIter;
use uom::si::length::millimeter;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum RandomParam {
    SideLengthX,
    SideLengthY,
    Points,
}

impl From<RandomParam> for InputParam {
    fn from(value: RandomParam) -> Self {
        match value {
            RandomParam::SideLengthX => Self::Length("Length X in mm".into()),
            RandomParam::SideLengthY => Self::Length("Length Y in mm".into()),
            RandomParam::Points => Self::Usize("#Points".into()),
        }
    }
}

impl IntoInputDataStrings<Random> for RandomParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::SideLengthX => "LengthX",
            Self::SideLengthY => "LengthY",
            Self::Points => "Points",
        };
        format!("random{id_str}Input")
    }

    fn create_value_string(&self, obj: &Random) -> String {
        match self {
            Self::SideLengthX => format!("{:.3e}", obj.side_length_x().get::<millimeter>()),
            Self::SideLengthY => format!("{:.3e}", obj.side_length_y().get::<millimeter>()),
            Self::Points => format!("{}", obj.nr_of_points()),
        }
    }
}

impl IntoInputData<f64, Random, PosDistType> for RandomParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut Random, f64) {
        match self {
            Self::SideLengthX => move |obj: &mut Random, val: f64| {
                let _ = obj.set_side_length_x(millimeter!(val));
            },
            Self::SideLengthY => move |obj: &mut Random, val: f64| {
                let _ = obj.set_side_length_y(millimeter!(val));
            },
            Self::Points => move |obj: &mut Random, val: f64| {
                if let Some(val) = try_f64_to_usize(val) {
                    let _ = obj.set_nr_of_points(val);
                }
            },
        }
    }
}
