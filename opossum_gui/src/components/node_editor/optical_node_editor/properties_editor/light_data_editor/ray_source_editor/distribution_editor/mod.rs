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
pub fn DistributionEditor(
    ray_data_builder_sig: ReadSignal<RayDataBuilder>,
    on_save: EventHandler<RayDataBuilder>,
) -> Element {
    rsx! {
        div {
            class: "accordion accordion-borderless bg-dark border-start",
            id: "accordionSourceDists",
            PositionDistributionEditor {
                pos_dist_type: ray_data_builder_sig.read().get_position_distribution_type().unwrap_or_default(),
                ray_data_builder_sig,
                on_save,
            }
            EnergyDistributionEditor {
                energy_dist_type: ray_data_builder_sig.read().get_energy_distribution_type().unwrap_or_default(),
                ray_data_builder_sig,
                on_save,
            }
            SpectralDistributionEditor {
                spect_dist_type: ray_data_builder_sig.read().get_spectral_distribution_type().unwrap_or_default(),
                ray_data_builder_sig,
                on_save,
            }
        }
    }
}
