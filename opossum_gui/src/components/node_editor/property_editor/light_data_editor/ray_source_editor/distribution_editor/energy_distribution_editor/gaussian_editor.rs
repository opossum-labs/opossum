use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::inputs::{InputData, InputParam, IntoInputData, IntoInputDataStrings},
};
use dioxus::prelude::*;
use opossum_backend::{
    EnergyDistType, EnergyDistribution, General2DGaussian, degree, joule, millimeter,
};
use strum::{EnumIter, IntoEnumIterator};
use uom::si::{angle::degree, energy::joule, length::millimeter};

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
            General2DGaussianParam::CenterX => Self::Length("Center X in mm".into()),
            General2DGaussianParam::CenterY => Self::Length("Center Y in mm".into()),
            General2DGaussianParam::SigmaX => Self::Length("Sigma X in mm".into()),
            General2DGaussianParam::SigmaY => Self::Length("Sigma Y in mm".into()),
            General2DGaussianParam::Energy => Self::Energy("Energy in J".into()),
            General2DGaussianParam::Power => Self::F64("Power".into()),
            General2DGaussianParam::Theta => Self::Angle("Theta in degrees".into()),
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
            Self::CenterX => format!("{:.3e}", obj.center().x.get::<millimeter>()),
            Self::CenterY => format!("{:.3e}", obj.center().y.get::<millimeter>()),
            Self::SigmaX => format!("{:.3e}", obj.sigma().x.get::<millimeter>()),
            Self::SigmaY => format!("{:.3e}", obj.sigma().y.get::<millimeter>()),
            Self::Energy => format!("{:.3}", obj.get_total_energy().get::<joule>()),
            Self::Power => format!("{}", obj.power()),
            Self::Theta => format!("{:.3}", obj.theta().get::<degree>()),
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
            Self::CenterX => {
                move |obj: &mut General2DGaussian, val: f64| obj.set_center_x(millimeter!(val))
            }
            Self::CenterY => {
                move |obj: &mut General2DGaussian, val: f64| obj.set_center_y(millimeter!(val))
            }
            Self::SigmaX => {
                move |obj: &mut General2DGaussian, val: f64| obj.set_sigma_x(millimeter!(val))
            }
            Self::SigmaY => {
                move |obj: &mut General2DGaussian, val: f64| obj.set_sigma_y(millimeter!(val))
            }
            Self::Energy => move |obj: &mut General2DGaussian, val: f64| {
                obj.set_energy(joule!(val)).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid energy value: {val}"));
                });
            },
            Self::Power => move |obj: &mut General2DGaussian, val: f64| obj.set_power(val),
            Self::Theta => move |obj: &mut General2DGaussian, val: f64| obj.set_theta(degree!(val)),
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
    energy_dist_type_sig: Signal<EnergyDistType>,
) -> Vec<InputData> {
    let mut input_data = Vec::<InputData>::new();
    for enum_variant in General2DGaussianParam::iter() {
        match enum_variant {
            General2DGaussianParam::Rectangular => {
                input_data.push(
                    IntoInputData::<bool, General2DGaussian, EnergyDistType>::to_input_data(
                        &enum_variant,
                        *gaussian,
                        energy_dist_type_sig,
                    ),
                );
            }
            _ => input_data.push(
                IntoInputData::<f64, General2DGaussian, EnergyDistType>::to_input_data(
                    &enum_variant,
                    *gaussian,
                    energy_dist_type_sig,
                ),
            ),
        }
    }
    input_data
}
