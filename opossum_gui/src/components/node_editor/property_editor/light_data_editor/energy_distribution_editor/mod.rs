#![allow(clippy::derive_partial_eq_without_eq)]
mod gaussian_editor;
mod uniform_editor;

use crate::components::node_editor::{
    accordion::AccordionItem,
    inputs::{
        input_components::{LabeledSelect, RowedInputs},
        select_options_from_enum_iterator, InputData, IntoInputData,
    },
    property_editor::light_data_editor::energy_distribution_editor::{
        gaussian_editor::get_general_2d_gaussian_input_params, uniform_editor::UniformParam,
    },
};
use dioxus::prelude::*;
use opossum_backend::{DefaultFromName, EnergyDistType};

#[component]
pub fn RayEnergyDistributionEditor(energy_dist_type_sig: Signal<EnergyDistType>) -> Element {
    let inputs: Vec<InputData> =
        get_energy_dist_input_data(energy_dist_type_sig(), energy_dist_type_sig);
    rsx! {
        RowedInputs { inputs }
    }
}

#[component]
pub fn EnergyDistributionEditor(energy_dist_type_sig: Signal<EnergyDistType>) -> Element {
    let accordion_item_content = rsx! {
        RayEnergyDistributionSelector { energy_dist_type_sig }
        RayEnergyDistributionEditor { energy_dist_type_sig }
    };

    rsx! {
        AccordionItem {
            elements: vec![accordion_item_content],
            header: "Energy Distribution",
            header_id: "sourceEnergyDistHeading",
            parent_id: "accordionSourceDists",
            content_id: "sourceEnergyDistCollapse",
        }
    }
}

#[component]
pub fn RayEnergyDistributionSelector(energy_dist_type_sig: Signal<EnergyDistType>) -> Element {
    rsx! {
        LabeledSelect {
            id: "selectRaysEnergyDistribution",
            label: "Rays Energy Distribution",
            options: select_options_from_enum_iterator(&*energy_dist_type_sig.read(), None),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(edt) = EnergyDistType::default_from_name(val.as_str()) {
                    energy_dist_type_sig.set(edt);
                }
            },
        }
    }
}

fn get_energy_dist_input_data(
    energy_dist_type: EnergyDistType,
    energy_dist_type_sig: Signal<EnergyDistType>,
) -> Vec<InputData> {
    match &energy_dist_type {
        EnergyDistType::Uniform(u) => UniformParam::to_input_data_vec(u, energy_dist_type_sig),
        EnergyDistType::General2DGaussian(g) => {
            get_general_2d_gaussian_input_params(g, energy_dist_type_sig)
        }
    }
}
