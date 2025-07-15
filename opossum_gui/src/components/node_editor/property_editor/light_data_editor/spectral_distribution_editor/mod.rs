#![allow(clippy::derive_partial_eq_without_eq)]
mod gaussian_editor;
mod laser_lines_editor;

use crate::components::node_editor::{
    accordion::AccordionItem,
    inputs::{
        input_components::{LabeledSelect, RowedInputs},
        select_options_from_enum_iterator, IntoInputData,
    },
    property_editor::light_data_editor::spectral_distribution_editor::{
        gaussian_editor::GaussianSpectrumParam, laser_lines_editor::LaserLineInput,
    },
};
use dioxus::prelude::*;
use opossum_backend::{DefaultFromName, SpecDistType};

#[component]
pub fn RaySpectralDistributionEditor(spect_dist_type_sig: Signal<SpecDistType>) -> Element {
    match spect_dist_type_sig() {
        SpecDistType::Gaussian(g) => {
            rsx! {
                RowedInputs { inputs: GaussianSpectrumParam::to_input_data_vec(&g, spect_dist_type_sig) }
            }
        }
        SpecDistType::LaserLines(laser_lines) => {
            rsx! {
                LaserLineInput { laser_lines, spect_dist_type_sig }
            }
        }
    }
}

#[component]
pub fn SpectralDistributionEditor(spect_dist_type_sig: Signal<SpecDistType>) -> Element {
    let accordion_item_content = rsx! {
        RaySpectralDistributionSelector { spect_dist_type_sig }
        RaySpectralDistributionEditor { spect_dist_type_sig }
    };

    rsx! {
        AccordionItem {
            elements: vec![accordion_item_content],
            header: "Spectral Distribution",
            header_id: "sourceSpectralDistHeading",
            parent_id: "accordionSourceDists",
            content_id: "sourceSpectralDistCollapse",
        }
    }
}

#[component]
pub fn RaySpectralDistributionSelector(spect_dist_type_sig: Signal<SpecDistType>) -> Element {
    rsx! {
        LabeledSelect {
            id: "selectRaysSpectralDistribution",
            label: "Rays Spectral Distribution",
            options: select_options_from_enum_iterator(
                &*spect_dist_type_sig.read(),
                None,
            ),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(sdt) = SpecDistType::default_from_name(val.as_str()) {
                    spect_dist_type_sig.set(sdt);
                }
            },
        }
    }
}
