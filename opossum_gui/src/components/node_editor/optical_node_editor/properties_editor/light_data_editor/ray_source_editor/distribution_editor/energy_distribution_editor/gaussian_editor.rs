use crate::components::{
    logger::LogResultExt,
    node_editor::inputs::{InputData, InputParam, IntoInputData, IntoInputDataStrings},
};
use dioxus::prelude::*;
use opossum_core::energy_distributions::{EnergyDistType, EnergyDistribution, General2DGaussian};
use opossum_core::{degree, joule, meter};
use strum::{EnumIter, IntoEnumIterator};
use uom::si::angle::degree;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum General2DGaussianParam {
    CenterX,
    CenterY,
    SigmaX,
    SigmaY,
    Energy,
    Power,
    Theta,
    Rectangular,
}

impl From<General2DGaussianParam> for InputParam {
    fn from(value: General2DGaussianParam) -> Self {
        match value {
            General2DGaussianParam::CenterX => Self::SIUnit("Center X".into(), "m".into()),
            General2DGaussianParam::CenterY => Self::SIUnit("Center Y".into(), "m".into()),
            General2DGaussianParam::SigmaX => Self::SIUnit("Sigma X".into(), "m".into()),
            General2DGaussianParam::SigmaY => Self::SIUnit("Sigma Y".into(), "m".into()),
            General2DGaussianParam::Energy => Self::SIUnit("Energy".into(), "J".into()),
            General2DGaussianParam::Power => Self::F64("Power".into()),
            General2DGaussianParam::Theta => Self::SIUnit("Theta".into(), "deg".into()),
            General2DGaussianParam::Rectangular => Self::Bool("Rectangular".into()),
        }
    }
}

impl IntoInputDataStrings<General2DGaussian> for General2DGaussianParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::CenterX => "CenterX",
            Self::CenterY => "CenterY",
            Self::SigmaX => "SigmaX",
            Self::SigmaY => "SigmaY",
            Self::Energy => "Energy",
            Self::Power => "Power",
            Self::Theta => "Theta",
            Self::Rectangular => "Rectangular",
        };
        format!("spatialGaussian{id_str}Input")
    }

    fn create_value_string(&self, obj: &General2DGaussian) -> String {
        match self {
            Self::CenterX => format!("{}", obj.center().x.value),
            Self::CenterY => format!("{}", obj.center().y.value),
            Self::SigmaX => format!("{}", obj.sigma().x.value),
            Self::SigmaY => format!("{}", obj.sigma().y.value),
            Self::Energy => format!("{}", obj.get_total_energy().value),
            Self::Power => format!("{}", obj.power()),
            Self::Theta => format!("{}", obj.theta().get::<degree>()),
            Self::Rectangular => format!("{}", obj.rectangular()),
        }
    }
}

impl IntoInputData<f64, General2DGaussian, EnergyDistType> for General2DGaussianParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut General2DGaussian, f64) {
        match self {
            Self::CenterX => move |obj: &mut General2DGaussian, val: f64| {
                obj.set_center_x(meter!(val))
                    .log_err_with_context("`set_center_x` of gaussian energy distribution");
            },
            Self::CenterY => move |obj: &mut General2DGaussian, val: f64| {
                obj.set_center_y(meter!(val))
                    .log_err_with_context("`set_center_y` of gaussian energy distribution");
            },
            Self::SigmaX => move |obj: &mut General2DGaussian, val: f64| {
                obj.set_sigma_x(meter!(val))
                    .log_err_with_context("`set_sigma_x` of gaussian energy distribution");
            },
            Self::SigmaY => move |obj: &mut General2DGaussian, val: f64| {
                obj.set_sigma_y(meter!(val))
                    .log_err_with_context("`set_sigma_y` of gaussian energy distribution");
            },
            Self::Energy => move |obj: &mut General2DGaussian, val: f64| {
                obj.set_energy(joule!(val))
                    .log_err_with_context("`set_energy` of gaussian energy distribution");
            },
            Self::Power => move |obj: &mut General2DGaussian, val: f64| {
                obj.set_power(val)
                    .log_err_with_context("`set_power` of gaussian energy distribution");
            },
            Self::Theta => move |obj: &mut General2DGaussian, val: f64| {
                obj.set_theta(degree!(val))
                    .log_err_with_context("`set_theta` of gaussian energy distribution");
            },
            Self::Rectangular => move |_: &mut General2DGaussian, _: f64| {},
        }
    }
}

impl IntoInputData<bool, General2DGaussian, EnergyDistType> for General2DGaussianParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<bool> {
        let e_value = e.value();
        e_value.parse::<bool>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut General2DGaussian, bool) {
        if Self::Rectangular == *self {
            move |obj: &mut General2DGaussian, val: bool| obj.set_rectangular(val)
        } else {
            move |_: &mut General2DGaussian, _: bool| {}
        }
    }
}

pub fn get_general_2d_gaussian_input_params(
    gaussian: &General2DGaussian,
    on_save: EventHandler<EnergyDistType>,
) -> Vec<InputData> {
    let mut input_data = Vec::<InputData>::new();
    for enum_variant in General2DGaussianParam::iter() {
        match enum_variant {
            General2DGaussianParam::Rectangular => {
                input_data.push(
                    IntoInputData::<bool, General2DGaussian, EnergyDistType>::to_input_data(
                        &enum_variant,
                        *gaussian,
                        on_save,
                    ),
                );
            }
            _ => input_data.push(
                IntoInputData::<f64, General2DGaussian, EnergyDistType>::to_input_data(
                    &enum_variant,
                    *gaussian,
                    on_save,
                ),
            ),
        }
    }
    input_data
}
