#![allow(clippy::derive_partial_eq_without_eq)]
mod energy_type_selection;
mod laser_line_editor;
mod spectrum_from_file_editor;
use dioxus::prelude::*;
use energy_type_selection::EnergyDataBuilderSelector;
use laser_line_editor::EnergyLaserLineEditor;
use opossum_core::prelude::{EnergyDataBuilder, LightDataBuilder};
use spectrum_from_file_editor::SpectrumFromFileEditor;

use crate::components::node_editor::optical_node_editor::properties_editor::use_update_signal_with_reactive_prop;

#[component]
pub fn EnergySourceEditor(
    energy_data_builder: EnergyDataBuilder,
    light_data_builder_sig: Signal<LightDataBuilder>,
) -> Element {
    let energy_data_builder_sig = use_signal(|| energy_data_builder.clone());
    use_update_signal_with_reactive_prop(energy_data_builder.clone(), energy_data_builder_sig);

    use_effect(move || {
        if energy_data_builder != *energy_data_builder_sig.read() {
            light_data_builder_sig.set(LightDataBuilder::Energy(
                energy_data_builder_sig.read().clone(),
            ));
        }
    });

    rsx! {
        EnergyDataBuilderSelector { energy_data_builder_sig }
        EnergyDataEditor { energy_data_builder_sig }
    }
}

#[component]
pub fn EnergyDataEditor(energy_data_builder_sig: Signal<EnergyDataBuilder>) -> Element {
    match energy_data_builder_sig() {
        EnergyDataBuilder::Raw(_) => rsx! {},
        EnergyDataBuilder::FromFile(spec_file) => rsx! {
            SpectrumFromFileEditor { spec_file, energy_data_builder_sig }
        },
        EnergyDataBuilder::LaserLines(energy_laser_lines) => rsx! {
            EnergyLaserLineEditor { energy_laser_lines, energy_data_builder_sig }
        },
    }
}
