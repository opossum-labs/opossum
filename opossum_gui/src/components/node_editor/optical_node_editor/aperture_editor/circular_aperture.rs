use crate::components::{
    logger::LogResultExt,
    node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings},
};
use dioxus::prelude::*;
use opossum_core::{
    apertures::RectangleShape, meter, prelude::ApertureShape, utils::try_f64_to_usize
};
use strum::EnumIter;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum CircularApertureParam {
    Width,
    Height,
    CenterX,
    CenterY
}


impl From<CircularApertureParam> for InputParam {
    fn from(value: CircularApertureParam) -> Self {
        match value {
            CircularApertureParam::Width => Self::SIUnit("Width".into(), "m".into()),
            CircularApertureParam::Height => Self::SIUnit("Height".into(), "m".into()),
            CircularApertureParam::CenterX => Self::SIUnit("Center X".into(), "m".into()),
            CircularApertureParam::CenterY => Self::SIUnit("Center Y".into(), "m".into()),
        }
    }
}

impl IntoInputDataStrings<CircularShape> for CircularApertureParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::Width => "Width",
            Self::Height => "Height",
            Self::CenterX => "CenterX",
            Self::CenterY => "CenterY",
        };
        format!("Grid{id_str}Input")
    }

    fn create_value_string(&self, obj: &CircularShape) -> String {
        match self {
            Self::Width => format!("{}", obj.width().value),
            Self::Height => format!("{}", obj.height().value),
            Self::CenterX => format!("{}", obj.center().x.value),
            Self::CenterY => format!("{}", obj.center().y.value),
        }
    }
}

impl IntoInputData<f64, CircularShape, ApertureShape> for CircularApertureParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut CircularShape, f64) {
        match self {
            Self::Width => move |obj: &mut CircularShape, val: f64| {
                obj.set_width(meter!(val))
                    .log_err_with_context("`set_width` of circular aperture");
            },
            Self::Height => move |obj: &mut CircularShape, val: f64| {
                obj.set_height(meter!(val))
                    .log_err_with_context("`set_height` of circular aperture");
            },
            Self::CenterX => move |obj: &mut CircularShape, val: f64| {
                obj.set_center_x(meter!(val))
                    .log_err_with_context("`set_center_x` of circular aperture");
            },
            Self::CenterY => move |obj: &mut CircularShape, val: f64| {
                obj.set_center_y(meter!(val))
                    .log_err_with_context("`set_center_y` of circular aperture");
            },            
        }
    }
}
