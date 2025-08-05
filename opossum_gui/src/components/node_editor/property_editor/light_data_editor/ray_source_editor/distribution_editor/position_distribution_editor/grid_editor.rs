use crate::components::node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings};
use dioxus::prelude::*;
use opossum_backend::{Grid, PosDistType, f64_to_usize, millimeter};
use strum::EnumIter;
use uom::si::length::millimeter;

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
            GridParam::LengthX => Self::Length("Length X in mm".into()),
            GridParam::LengthY => Self::Length("Length Y in mm".into()),
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
            Self::NrOfPointsX => format!("{}", obj.nr_of_points().0),
            Self::NrOfPointsY => format!("{}", obj.nr_of_points().1),
            Self::LengthX => format!("{:.3e}", obj.side_length_x().get::<millimeter>()),
            Self::LengthY => format!("{:.3e}", obj.side_length_y().get::<millimeter>()),
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
            Self::NrOfPointsX => {
                move |obj: &mut Grid, val: f64| obj.set_nr_of_points_x(f64_to_usize(val))
            }
            Self::NrOfPointsY => {
                move |obj: &mut Grid, val: f64| obj.set_nr_of_points_y(f64_to_usize(val))
            }
            Self::LengthX => {
                move |obj: &mut Grid, val: f64| obj.set_side_length_x(millimeter!(val))
            }
            Self::LengthY => {
                move |obj: &mut Grid, val: f64| obj.set_side_length_y(millimeter!(val))
            }
        }
    }
}
