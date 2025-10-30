#![allow(clippy::derive_partial_eq_without_eq)]
mod gaussian_editor;
mod laser_lines_editor;

use crate::components::node_editor::{
    accordion::AccordionItem,
    inputs::{
        IntoInputData,
        input_components::{LabeledSelect, RowedInputs},
        select_options_from_enum_iterator,
    },
    optical_node_editor::properties_editor::use_update_signal_with_reactive_prop,
};
use dioxus::prelude::*;
use gaussian_editor::GaussianSpectrumParam;
use laser_lines_editor::LaserLineInput;
use opossum_core::{
    prelude::RayDataBuilder, spectral_distribution::SpecDistType,
    utils::default_from_name::DefaultFromName,
};

#[component]
pub fn RaySpectralDistributionEditor(spect_dist_type_sig: Signal<SpecDistType>) -> Element {
    match &*spect_dist_type_sig.read() {
        SpecDistType::Gaussian(g) => {
            rsx! {
                RowedInputs { inputs: GaussianSpectrumParam::to_input_data_vec(g, spect_dist_type_sig) }
            }
        }
        SpecDistType::LaserLines(laser_lines) => {
            rsx! {
                LaserLineInput { laser_lines: laser_lines.clone(), spect_dist_type_sig }
            }
        }
    }
}

#[component]
pub fn SpectralDistributionEditor(spect_dist_type: SpecDistType) -> Element {
    let mut ray_data_builder_sig = use_context::<Signal<RayDataBuilder>>();

    let spect_dist_type_sig = use_signal(|| spect_dist_type.clone());
    use_update_signal_with_reactive_prop(spect_dist_type, spect_dist_type_sig);

    use_effect(move || {
        ray_data_builder_sig
            .write()
            .set_spectral_dist(spect_dist_type_sig.read().clone());
    });

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
            options: select_options_from_enum_iterator(&*spect_dist_type_sig.read(), None),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(sdt) = SpecDistType::default_from_name(val.as_str()) {
                    spect_dist_type_sig.set(sdt);
                }
            },
        }
    }
}
