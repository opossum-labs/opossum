use crate::components::node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings};
use dioxus::prelude::*;
use opossum_backend::{FibonacciEllipse, PosDistType, f64_to_usize, millimeter};
use strum::EnumIter;
use uom::si::length::millimeter;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum FibonacciEllipseParam {
    MajorAxis,
    MinorAxis,
    Points,
}

impl From<FibonacciEllipseParam> for InputParam {
    fn from(value: FibonacciEllipseParam) -> Self {
        match value {
            FibonacciEllipseParam::MajorAxis => Self::Length("Major axis in mm".into()),
            FibonacciEllipseParam::MinorAxis => Self::Length("Minor axis in mm".into()),
            FibonacciEllipseParam::Points => Self::Usize("Number of points".into()),
        }
    }
}

impl IntoInputDataStrings<FibonacciEllipse> for FibonacciEllipseParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::MajorAxis => "MajorAxis",
            Self::MinorAxis => "MinorAxis",
            Self::Points => "Points",
        };
        format!("fibonacciEllipse{id_str}Input")
    }
    fn create_value_string(&self, obj: &FibonacciEllipse) -> String {
        match self {
            Self::MajorAxis => format!("{:.3e}", obj.radius_x().get::<millimeter>()),
            Self::MinorAxis => format!("{:.3e}", obj.radius_y().get::<millimeter>()),
            Self::Points => format!("{}", obj.nr_of_points()),
        }
    }
}

impl IntoInputData<f64, FibonacciEllipse, PosDistType> for FibonacciEllipseParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut FibonacciEllipse, f64) {
        match self {
            Self::MajorAxis => {
                move |obj: &mut FibonacciEllipse, val: f64| obj.set_radius_x(millimeter!(val))
            }
            Self::MinorAxis => {
                move |obj: &mut FibonacciEllipse, val: f64| obj.set_radius_y(millimeter!(val))
            }
            Self::Points => {
                move |obj: &mut FibonacciEllipse, val: f64| obj.set_nr_of_points(f64_to_usize(val))
            }
        }
    }
}
