mod gaussian_editor;
mod laser_lines_editor;

use crate::components::node_editor::{
    accordion::AccordionItem, analyzer_node_editor::light_data_editor::{default_gaussian, default_ray_laser_lines}, hooks::use_synced_signal, inputs::{
        IntoInputData,
        input_components::{LabeledSelect, RowedInputs},
        select_options_from_enum_iterator,
    },
};
use dioxus::prelude::*;
use gaussian_editor::GaussianSpectrumParam;
use laser_lines_editor::LaserLineInput;
use opossum_core::{
    distributions::spectral::SpecDistType, prelude::RayDataSource,
    utils::default_from_name::DefaultFromName,
};

#[component]
pub fn RaySpectralDistributionEditor(
    spect_dist_type_sig: ReadSignal<SpecDistType>,
    on_save: EventHandler<SpecDistType>,
    readonly: bool,
) -> Element {
    match &*spect_dist_type_sig.read() {
        SpecDistType::Gaussian(g) => {
            rsx! {
                RowedInputs { inputs: GaussianSpectrumParam::to_input_data_vec(g, on_save, readonly) }
            }
        }
        SpecDistType::LaserLines(laser_lines) => {
            rsx! {
                LaserLineInput {
                    laser_lines: laser_lines.clone(),
                    on_save,
                    readonly,
                }
            }
        }
    }
}

#[component]
pub fn SpectralDistributionEditor(
    spect_dist_type: SpecDistType,
    ray_data_builder_sig: ReadSignal<RayDataSource>,
    on_save: EventHandler<RayDataSource>,
    readonly: bool,
) -> Element {
    let mut spect_dist_type_sig = use_synced_signal(spect_dist_type);

    let on_spect_dist_save = EventHandler::new(move |new_spect_dist_type: SpecDistType| {
        spect_dist_type_sig.set(new_spect_dist_type);
        let mut ray_data_builder = ray_data_builder_sig.read().clone();
        ray_data_builder.set_spectral_dist(spect_dist_type_sig.read().clone());
        on_save.call(ray_data_builder);
    });

    let accordion_item_content = rsx! {
        RaySpectralDistributionSelector {
            spect_dist_type_sig,
            on_save: on_spect_dist_save,
            readonly,
        }
        RaySpectralDistributionEditor {
            spect_dist_type_sig,
            on_save: on_spect_dist_save,
            readonly,
        }
    };

    rsx! {
        AccordionItem {
            elements: vec![accordion_item_content],
            header: "Spectral Distribution",
            header_id: "sourceSpectralDistHeading",
            parent_id: "accordionSourceDists",
            content_id: "sourceSpectralDistCollapse",
            level: 2,
        }
    }
}

#[component]
pub fn RaySpectralDistributionSelector(
    spect_dist_type_sig: ReadSignal<SpecDistType>,
    on_save: EventHandler<SpecDistType>,
    readonly: bool,
) -> Element {
    rsx! {
        LabeledSelect {
            id: "selectRaysSpectralDistribution",
            label: "Rays Spectral Distribution",
            options: select_options_from_enum_iterator(&*spect_dist_type_sig.read(), None),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(sdt) = SpecDistType::default_from_name(val.as_str()) {
                    let default_wvl = crate::APP_CONFIG.read().default_wavelength();
                    let configured_sdt = match sdt {
                        SpecDistType::Gaussian(_) => {
                            SpecDistType::Gaussian(default_gaussian(default_wvl))
                        }
                        SpecDistType::LaserLines(_) => {
                            SpecDistType::LaserLines(default_ray_laser_lines(default_wvl))
                        }
                    };
                    on_save.call(configured_sdt);
                }
            },
        }
    }
}
