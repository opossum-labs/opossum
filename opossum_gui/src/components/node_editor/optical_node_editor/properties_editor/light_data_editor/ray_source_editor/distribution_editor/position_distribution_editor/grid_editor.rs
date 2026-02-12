use crate::components::{
    logger::LogResultExt,
    node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings},
};
use dioxus::prelude::*;
use opossum_core::{
    meter,
    position_distributions::{Grid, PosDistType},
    utils::try_f64_to_usize,
};
use strum::EnumIter;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum GridParam {
    LengthX,
    LengthY,
    NrOfPointsX,
    NrOfPointsY,
}

impl From<GridParam> for InputParam {
    fn from(value: GridParam) -> Self {
        match value {
            GridParam::NrOfPointsX => Self::Usize("#Points X".into()),
            GridParam::NrOfPointsY => Self::Usize("#Points Y".into()),
            GridParam::LengthX => Self::SIUnit("Length X".into(), "m".into()),
            GridParam::LengthY => Self::SIUnit("Length Y".into(), "m".into()),
        }
    }
}

impl IntoInputDataStrings<Grid> for GridParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::NrOfPointsX => "NrOfPointsX",
            Self::NrOfPointsY => "NrOfPointsY",
            Self::LengthX => "LengthX",
            Self::LengthY => "LengthY",
        };
        format!("Grid{id_str}Input")
    }

    fn create_value_string(&self, obj: &Grid) -> String {
        match self {
            Self::NrOfPointsX => format!("{}", obj.nr_of_points_x()),
            Self::NrOfPointsY => format!("{}", obj.nr_of_points_y()),
            Self::LengthX => format!("{}", obj.side_length_x().value),
            Self::LengthY => format!("{}", obj.side_length_y().value),
        }
    }
}

impl IntoInputData<f64, Grid, PosDistType> for GridParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut Grid, f64) {
        match self {
            Self::NrOfPointsX => move |obj: &mut Grid, val: f64| {
                if let Some(val) = try_f64_to_usize(val) {
                    obj.set_nr_of_points_x(val)
                        .log_err_with_context("`set_nr_of_points_x` of grid");
                }
            },
            Self::NrOfPointsY => move |obj: &mut Grid, val: f64| {
                if let Some(val) = try_f64_to_usize(val) {
                    obj.set_nr_of_points_y(val)
                        .log_err_with_context("`set_nr_of_points_y` of grid");
                }
            },
            Self::LengthX => move |obj: &mut Grid, val: f64| {
                obj.set_side_length_x(meter!(val))
                    .log_err_with_context("`set_side_length_x` of grid");
            },
            Self::LengthY => move |obj: &mut Grid, val: f64| {
                obj.set_side_length_y(meter!(val))
                    .log_err_with_context("`set_side_length_y` of grid");
            },
        }
    }
}
