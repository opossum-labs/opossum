use crate::components::{
    logger::LogResultExt,
    node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings},
};
use dioxus::prelude::*;
use opossum_core::{
    meter,
    spectral_distribution::{Gaussian, SpecDistType},
    utils::try_f64_to_usize,
};
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
            GaussianSpectrumParam::CenterWavelength => Self::Length("Center λ".into()),
            GaussianSpectrumParam::Fwhm => Self::Length("FWHM".into()),
            GaussianSpectrumParam::Power => Self::F64("Power".into()),
            GaussianSpectrumParam::WavelengthStart => Self::Length("Start λ".into()),
            GaussianSpectrumParam::WavelengthEnd => Self::Length("End λ".into()),
            GaussianSpectrumParam::NumPoints => Self::Usize("#Points".into()),
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
            Self::CenterWavelength => format!("{}", obj.mu().value),
            Self::Fwhm => format!("{}", obj.fwhm().value),
            Self::Power => format!("{}", obj.power()),
            Self::WavelengthStart => format!("{}", obj.wvl_start().value),
            Self::WavelengthEnd => format!("{}", obj.wvl_end().value),
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
            Self::CenterWavelength => move |obj: &mut Gaussian, val: f64| {
                obj.set_mu(meter!(val))
                    .log_err_with_context("`set_mu` of spectral gaussian distribution");
            },
            Self::Fwhm => move |obj: &mut Gaussian, val: f64| {
                obj.set_fwhm(meter!(val))
                    .log_err_with_context("`set_fwhm` of spectral gaussian distribution");
            },
            Self::Power => move |obj: &mut Gaussian, val: f64| {
                obj.set_power(val)
                    .log_err_with_context("`set_power` of spectral gaussian distribution");
            },
            Self::WavelengthStart => move |obj: &mut Gaussian, val: f64| {
                obj.set_wvl_start(meter!(val))
                    .log_err_with_context("`set_wvl_start` of spectral gaussian distribution");
            },
            Self::WavelengthEnd => move |obj: &mut Gaussian, val: f64| {
                obj.set_wvl_end(meter!(val))
                    .log_err_with_context("`set_wvl_end` of spectral gaussian distribution");
            },
            Self::NumPoints => move |obj: &mut Gaussian, val: f64| {
                if let Some(val) = try_f64_to_usize(val) {
                    obj.set_num_points(val)
                        .log_err_with_context("`set_num_points` of spectral gaussian distribution");
                }
            },
        }
    }
}
