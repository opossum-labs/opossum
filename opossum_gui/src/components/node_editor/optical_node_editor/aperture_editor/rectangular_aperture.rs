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
pub enum RectApertureParam {
    Width,
    Height,
    CenterX,
    CenterY
}


impl From<RectApertureParam> for InputParam {
    fn from(value: RectApertureParam) -> Self {
        match value {
            RectApertureParam::Width => Self::SIUnit("Width".into(), "m".into()),
            RectApertureParam::Height => Self::SIUnit("Height".into(), "m".into()),
            RectApertureParam::CenterX => Self::SIUnit("Center X".into(), "m".into()),
            RectApertureParam::CenterY => Self::SIUnit("Center Y".into(), "m".into()),
        }
    }
}

impl IntoInputDataStrings<RectangleShape> for RectApertureParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::Width => "Width",
            Self::Height => "Height",
            Self::CenterX => "CenterX",
            Self::CenterY => "CenterY",
        };
        format!("Grid{id_str}Input")
    }

    fn create_value_string(&self, obj: &RectangleShape) -> String {
        match self {
            Self::Width => format!("{}", obj.width().value),
            Self::Height => format!("{}", obj.height().value),
            Self::CenterX => format!("{}", obj.center().x.value),
            Self::CenterY => format!("{}", obj.center().y.value),
        }
    }
}

impl IntoInputData<f64, RectangleShape, ApertureShape> for RectApertureParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut RectangleShape, f64) {
        match self {
            Self::Width => move |obj: &mut RectangleShape, val: f64| {
                obj.set_width(meter!(val))
                    .log_err_with_context("`set_width` of rectangle");
            },
            Self::Height => move |obj: &mut RectangleShape, val: f64| {
                obj.set_height(meter!(val))
                    .log_err_with_context("`set_height` of rectangle");
            },
            Self::CenterX => move |obj: &mut RectangleShape, val: f64| {
                obj.set_center_x(meter!(val))
                    .log_err_with_context("`set_center_x` of rectangle");
            },
            Self::CenterY => move |obj: &mut RectangleShape, val: f64| {
                obj.set_center_y(meter!(val))
                    .log_err_with_context("`set_center_y` of rectangle");
            },            
        }
    }
}
