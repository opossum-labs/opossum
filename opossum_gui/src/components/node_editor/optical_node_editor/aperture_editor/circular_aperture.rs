use crate::components::{
    logger::LogResultExt,
    node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings},
};
use dioxus::prelude::*;
use opossum_core::{apertures::CircleShape, meter, prelude::ApertureShape};
use opossum_core::{apertures::CircleShape, meter, prelude::ApertureShape};
use strum::EnumIter;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum CircularApertureParam {
    Radius,
}

impl From<CircularApertureParam> for InputParam {
    fn from(value: CircularApertureParam) -> Self {
        match value {
            CircularApertureParam::Radius => Self::SIUnit("Radius".into(), "m".into()),
        }
    }
}

impl IntoInputDataStrings<CircleShape> for CircularApertureParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::Radius => "Radius",
        };
        format!("Grid{id_str}Input")
    }

    fn create_value_string(&self, obj: &CircleShape) -> String {
        match self {
            Self::Radius => format!("{}", obj.radius().value),
        }
    }
}

impl IntoInputData<f64, CircleShape, ApertureShape> for CircularApertureParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut CircleShape, f64) {
        match self {
            Self::Radius => move |obj: &mut CircleShape, val: f64| {
                obj.set_radius(meter!(val))
                    .log_err_with_context("`set_radius` of circular aperture");
            },
        }
    }
}
