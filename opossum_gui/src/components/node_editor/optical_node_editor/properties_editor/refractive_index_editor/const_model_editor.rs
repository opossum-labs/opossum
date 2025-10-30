use crate::components::node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings};
use dioxus::prelude::*;
use opossum_core::{prelude::RefrIndexConst, refractive_index::RefractiveIndexType};
use strum::EnumIter;

#[derive(Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum ConstRefParam {
    RefractiveIndex,
}

impl From<ConstRefParam> for InputParam {
    fn from(_: ConstRefParam) -> Self {
        Self::F64("Refractive Index".into())
    }
}

impl IntoInputDataStrings<RefrIndexConst> for ConstRefParam {
    fn create_id_string(&self) -> String {
        "refractiveIndexConstInput".to_string()
    }
    fn create_value_string(&self, obj: &RefrIndexConst) -> String {
        format!("{:.3e}", obj.refractive_index())
    }
}

impl IntoInputData<f64, RefrIndexConst, RefractiveIndexType> for ConstRefParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut RefrIndexConst, f64) {
        move |obj: &mut RefrIndexConst, val: f64| obj.set_refractive_index(val)
    }
}
