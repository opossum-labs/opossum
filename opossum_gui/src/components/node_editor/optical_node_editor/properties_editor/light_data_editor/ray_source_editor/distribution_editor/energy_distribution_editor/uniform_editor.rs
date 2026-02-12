use crate::components::{
    logger::LogResultExt,
    node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings},
};
use dioxus::prelude::*;
use opossum_core::energy_distributions::{EnergyDistType, EnergyDistribution, UniformDist};
use opossum_core::joule;
use strum::EnumIter;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum UniformParam {
    Energy,
}

impl From<UniformParam> for InputParam {
    fn from(_: UniformParam) -> Self {
        Self::SIUnit("Energy".into(), "J".into())
    }
}

impl IntoInputDataStrings<UniformDist> for UniformParam {
    fn create_id_string(&self) -> String {
        "spatialGaussianEnergyInput".to_string()
    }

    fn create_value_string(&self, obj: &UniformDist) -> String {
        format!("{}", obj.get_total_energy().value)
    }
}

impl IntoInputData<f64, UniformDist, EnergyDistType> for UniformParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut UniformDist, f64) {
        move |obj: &mut UniformDist, val: f64| {
            obj.set_energy(joule!(val))
                .log_err_with_context("`set_energy` of gaussian energy distribution");
        }
    }
}
