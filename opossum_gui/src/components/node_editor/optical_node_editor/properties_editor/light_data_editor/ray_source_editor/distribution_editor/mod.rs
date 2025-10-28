#![allow(clippy::derive_partial_eq_without_eq)]
mod energy_distribution_editor;
mod position_distribution_editor;
mod spectral_distribution_editor;

use energy_distribution_editor::EnergyDistributionEditor;
use opossum_core::prelude::RayDataBuilder;
use position_distribution_editor::PositionDistributionEditor;
use spectral_distribution_editor::SpectralDistributionEditor;

use dioxus::prelude::*;

#[component]
pub fn DistributionEditor() -> Element {
    let ray_data_builder_sig = use_context::<Signal<RayDataBuilder>>();

    // let pos_dist_type_sig = use_signal(move || {
    //     ray_data_builder_sig
    //         .read()
    //         .get_position_distribution_type()
    //         .unwrap_or_default()
    // });
    // let energy_dist_type_sig = use_signal(move || {
    //     ray_data_builder_sig
    //         .read()
    //         .get_energy_distribution_type()
    //         .unwrap_or_default()
    // });
    // let spect_dist_type_sig = use_signal(move || {
    //     ray_data_builder_sig
    //         .read()
    //         .get_spectral_distribution_type()
    //         .unwrap_or_default()
    // });

    // use_effect(move || {
    //     ray_data_builder_sig
    //         .write()
    //         .set_energy_dist(*energy_dist_type_sig.read());
    // });

    // use_effect(move || {
    //     ray_data_builder_sig
    //         .write()
    //         .set_spectral_dist(spect_dist_type_sig.read().clone());
    // });

    rsx! {
        div {
            class: "accordion accordion-borderless bg-dark border-start",
            id: "accordionSourceDists",
            PositionDistributionEditor { pos_dist_type: ray_data_builder_sig.read().get_position_distribution_type().unwrap_or_default() }
            EnergyDistributionEditor { energy_dist_type: ray_data_builder_sig.read().get_energy_distribution_type().unwrap_or_default() }
            SpectralDistributionEditor { spect_dist_type: ray_data_builder_sig.read().get_spectral_distribution_type().unwrap_or_default() }
        }
    }
}
