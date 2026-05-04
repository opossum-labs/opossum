use crate::components::{
    logger::LogResultExt,
    node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings},
};
use dioxus::prelude::*;
use opossum_core::{apertures::GaussianShape, meter, prelude::ApertureShape};
use strum::EnumIter;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum GaussianApertureParam {
    Width,
    Height,
}

impl From<GaussianApertureParam> for InputParam {
    fn from(value: GaussianApertureParam) -> Self {
        match value {
            GaussianApertureParam::Width => Self::SIUnit("Width".into(), "m".into()),
            GaussianApertureParam::Height => Self::SIUnit("Height".into(), "m".into()),
        }
    }
}

impl IntoInputDataStrings<GaussianShape> for GaussianApertureParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::Width => "Width",
            Self::Height => "Height",
        };
        format!("Grid{id_str}Input")
    }

    fn create_value_string(&self, obj: &GaussianShape) -> String {
        match self {
            Self::Width => format!("{}", obj.sigma().0.value),
            Self::Height => format!("{}", obj.sigma().1.value),
        }
    }
}

impl IntoInputData<f64, GaussianShape, ApertureShape> for GaussianApertureParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut GaussianShape, f64) {
        match self {
            Self::Width => move |obj: &mut GaussianShape, val: f64| {
                obj.set_sigma_x(meter!(val))
                    .log_err_with_context("`set_sigma_x` of gaussian");
            },
            Self::Height => move |obj: &mut GaussianShape, val: f64| {
                obj.set_sigma_y(meter!(val))
                    .log_err_with_context("`set_sigma_y` of gaussian");
            },
        }
    }
}
