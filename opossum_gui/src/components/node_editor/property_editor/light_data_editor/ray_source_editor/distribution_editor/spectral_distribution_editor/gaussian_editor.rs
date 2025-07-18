use crate::components::node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings};
use dioxus::prelude::*;
use opossum_backend::{f64_to_usize, millimeter, Gaussian, SpecDistType};
use strum::EnumIter;
use uom::si::length::nanometer;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum GaussianSpectrumParam {
    CenterWavelength,
    Fwhm,
    Power,
    WavelengthStart,
    WavelengthEnd,
    NumPoints,
}

impl From<GaussianSpectrumParam> for InputParam {
    fn from(value: GaussianSpectrumParam) -> Self {
        match value {
            GaussianSpectrumParam::CenterWavelength => {
                Self::Length("Center wavelength in nm".into())
            }
            GaussianSpectrumParam::Fwhm => Self::Length("FWHM in nm".into()),
            GaussianSpectrumParam::Power => Self::F64("Power".into()),
            GaussianSpectrumParam::WavelengthStart => Self::Length("Start wavelength in nm".into()),
            GaussianSpectrumParam::WavelengthEnd => Self::Length("End wavelength in nm".into()),
            GaussianSpectrumParam::NumPoints => Self::Usize("Number of points".into()),
        }
    }
}

impl IntoInputDataStrings<Gaussian> for GaussianSpectrumParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::CenterWavelength => "CenterWavelength",
            Self::Fwhm => "FWHM",
            Self::Power => "Power",
            Self::WavelengthStart => "WavelengthStart",
            Self::WavelengthEnd => "WavelengthEnd",
            Self::NumPoints => "NumPoints",
        };
        format!("spectralGaussian{id_str}Input")
    }

    fn create_value_string(&self, obj: &Gaussian) -> String {
        match self {
            Self::CenterWavelength => format!("{:.3e}", obj.mu().get::<nanometer>()),
            Self::Fwhm => format!("{:.3e}", obj.fwhm().get::<nanometer>()),
            Self::Power => format!("{}", obj.power()),
            Self::WavelengthStart => format!("{:.3e}", obj.wvl_start().get::<nanometer>()),
            Self::WavelengthEnd => format!("{:.3e}", obj.wvl_end().get::<nanometer>()),
            Self::NumPoints => format!("{}", obj.num_points()),
        }
    }
}

impl IntoInputData<f64, Gaussian, SpecDistType> for GaussianSpectrumParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut Gaussian, f64) {
        match self {
            Self::CenterWavelength => {
                move |obj: &mut Gaussian, val: f64| obj.set_mu(millimeter!(val))
            }
            Self::Fwhm => move |obj: &mut Gaussian, val: f64| obj.set_fwhm(millimeter!(val)),
            Self::Power => move |obj: &mut Gaussian, val: f64| obj.set_power(val),
            Self::WavelengthStart => {
                move |obj: &mut Gaussian, val: f64| obj.set_wvl_start(millimeter!(val))
            }
            Self::WavelengthEnd => {
                move |obj: &mut Gaussian, val: f64| obj.set_wvl_end(millimeter!(val))
            }
            Self::NumPoints => {
                move |obj: &mut Gaussian, val: f64| obj.set_num_points(f64_to_usize(val))
            }
        }
    }
}
