use crate::components::node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings};
use dioxus::prelude::*;
use opossum_backend::{f64_to_usize, millimeter, FibonacciRectangle, PosDistType};
use strum::EnumIter;
use uom::si::length::millimeter;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum FibonacciRectParam {
    SideLengthX,
    SideLengthY,
    Points,
}

impl From<FibonacciRectParam> for InputParam {
    fn from(value: FibonacciRectParam) -> Self {
        match value {
            FibonacciRectParam::SideLengthX => Self::Length("Length X in mm".into()),
            FibonacciRectParam::SideLengthY => Self::Length("Length Y in mm".into()),
            FibonacciRectParam::Points => Self::Usize("Number of points".into()),
        }
    }
}

impl IntoInputDataStrings<FibonacciRectangle> for FibonacciRectParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::SideLengthX => "LengthX",
            Self::SideLengthY => "LengthY",
            Self::Points => "Points",
        };
        format!("fibonacciRect{id_str}Input")
    }

    fn create_value_string(&self, obj: &FibonacciRectangle) -> String {
        match self {
            Self::SideLengthX => format!("{:.3e}", obj.side_length_x().get::<millimeter>()),
            Self::SideLengthY => format!("{:.3e}", obj.side_length_y().get::<millimeter>()),
            Self::Points => format!("{}", obj.nr_of_points()),
        }
    }
}

impl IntoInputData<f64, FibonacciRectangle, PosDistType> for FibonacciRectParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut FibonacciRectangle, f64) {
        match self {
            Self::SideLengthX => move |obj: &mut FibonacciRectangle, val: f64| {
                obj.set_side_length_x(millimeter!(val));
            },
            Self::SideLengthY => move |obj: &mut FibonacciRectangle, val: f64| {
                obj.set_side_length_y(millimeter!(val));
            },
            Self::Points => move |obj: &mut FibonacciRectangle, val: f64| {
                obj.set_nr_of_points(f64_to_usize(val));
            },
        }
    }
}
