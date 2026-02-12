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
};
use dioxus::prelude::*;
use gaussian_editor::GaussianSpectrumParam;
use laser_lines_editor::LaserLineInput;
use opossum_core::{
    prelude::RayDataBuilder, spectral_distribution::SpecDistType,
    utils::default_from_name::DefaultFromName,
};

#[component]
pub fn RaySpectralDistributionEditor(
    spect_dist_type_sig: ReadSignal<SpecDistType>,
    on_save: EventHandler<SpecDistType>,
) -> Element {
    match &*spect_dist_type_sig.read() {
        SpecDistType::Gaussian(g) => {
            rsx! {
                RowedInputs { inputs: GaussianSpectrumParam::to_input_data_vec(g, on_save) }
            }
        }
        SpecDistType::LaserLines(laser_lines) => {
            rsx! {
                LaserLineInput { laser_lines: laser_lines.clone(), on_save }
            }
        }
    }
}

#[component]
pub fn SpectralDistributionEditor(
    spect_dist_type: SpecDistType,
    ray_data_builder_sig: ReadSignal<RayDataBuilder>,
    on_save: EventHandler<RayDataBuilder>,
) -> Element {
    let mut spect_dist_type_sig = use_signal(|| spect_dist_type.clone());

    let on_spect_dist_save = EventHandler::new(move |new_spect_dist_type: SpecDistType| {
        spect_dist_type_sig.set(new_spect_dist_type);
        let mut ray_data_builder = ray_data_builder_sig.read().clone();
        ray_data_builder.set_spectral_dist(spect_dist_type_sig.read().clone());
        on_save.call(ray_data_builder);
    });

    let accordion_item_content = rsx! {
        RaySpectralDistributionSelector { spect_dist_type_sig, on_save: on_spect_dist_save }
        RaySpectralDistributionEditor { spect_dist_type_sig, on_save: on_spect_dist_save }
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
pub fn RaySpectralDistributionSelector(
    spect_dist_type_sig: ReadSignal<SpecDistType>,
    on_save: EventHandler<SpecDistType>,
) -> Element {
    rsx! {
        LabeledSelect {
            id: "selectRaysSpectralDistribution",
            label: "Rays Spectral Distribution",
            options: select_options_from_enum_iterator(&*spect_dist_type_sig.read(), None),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(sdt) = SpecDistType::default_from_name(val.as_str()) {
                    on_save.call(sdt);
                }
            },
        }
    }
}
