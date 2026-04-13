use crate::components::{
    logger::LogResultExt,
    node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings},
};
use dioxus::prelude::*;
use opossum_core::{
    distributions::position::{PosDistType, SobolDist},
    meter,
    utils::try_f64_to_usize,
};
use strum::EnumIter;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum SobolParam {
    SideLengthX,
    SideLengthY,
    Points,
}

impl From<SobolParam> for InputParam {
    fn from(value: SobolParam) -> Self {
        match value {
            SobolParam::SideLengthX => Self::SIUnit("Length X ".into(), "m".into()),
            SobolParam::SideLengthY => Self::SIUnit("Length Y ".into(), "m".into()),
            SobolParam::Points => Self::Usize("#Points".into()),
        }
    }
}

impl IntoInputDataStrings<SobolDist> for SobolParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::SideLengthX => "LengthX",
            Self::SideLengthY => "LengthY",
            Self::Points => "Points",
        };
        format!("sobol{id_str}Input")
    }

    fn create_value_string(&self, obj: &SobolDist) -> String {
        match self {
            Self::SideLengthX => format!("{}", obj.side_length_x().value),
            Self::SideLengthY => format!("{}", obj.side_length_y().value),
            Self::Points => format!("{}", obj.nr_of_points()),
        }
    }
}

impl IntoInputData<f64, SobolDist, PosDistType> for SobolParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut SobolDist, f64) {
        match self {
            Self::SideLengthX => move |obj: &mut SobolDist, val: f64| {
                obj.set_side_length_x(meter!(val))
                    .log_err_with_context("`set_side_length_x` of sobol");
            },
            Self::SideLengthY => move |obj: &mut SobolDist, val: f64| {
                obj.set_side_length_y(meter!(val))
                    .log_err_with_context("`set_side_length_y` of sobol");
            },
            Self::Points => move |obj: &mut SobolDist, val: f64| {
                if let Some(val) = try_f64_to_usize(val) {
                    obj.set_nr_of_points(val)
                        .log_err_with_context("`set_nr_of_points` of sobol");
                }
            },
        }
    }
}
