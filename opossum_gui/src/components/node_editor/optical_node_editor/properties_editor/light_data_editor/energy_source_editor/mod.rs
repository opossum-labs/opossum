#![allow(clippy::derive_partial_eq_without_eq)]
mod energy_type_selection;
mod laser_line_editor;
mod spectrum_from_file_editor;
use dioxus::prelude::*;
use energy_type_selection::EnergyDataBuilderSelector;
use laser_line_editor::EnergyLaserLineEditor;
use opossum_core::prelude::{EnergyDataBuilder, LightDataBuilder};
use spectrum_from_file_editor::SpectrumFromFileEditor;

#[component]
pub fn EnergySourceEditor(
    energy_data_builder: EnergyDataBuilder,
    on_save: EventHandler<LightDataBuilder>,
        readonly: bool
) -> Element {
    let mut energy_data_builder_sig = use_signal(|| energy_data_builder.clone());

    let on_energy_data_builder_save = EventHandler::new(move |new_builder: EnergyDataBuilder| {
        if new_builder != *energy_data_builder_sig.read() {
            on_save.call(new_builder.clone().into());
            energy_data_builder_sig.set(new_builder);
        }
    });

    rsx! {
        EnergyDataBuilderSelector {
            energy_data_builder_sig,
            on_energy_data_builder_save,
            readonly,
        }
        EnergyDataEditor {
            energy_data_builder_sig,
            on_save: on_energy_data_builder_save,
            readonly,
        }
    }
}

#[component]
pub fn EnergyDataEditor(
    energy_data_builder_sig: ReadSignal<EnergyDataBuilder>,
    on_save: EventHandler<EnergyDataBuilder>,
        readonly: bool
) -> Element {
    match energy_data_builder_sig() {
        EnergyDataBuilder::Raw(_) => rsx! {},
        EnergyDataBuilder::FromFile(spec_file) => rsx! {
            SpectrumFromFileEditor { spec_file, on_save, readonly }
        },
        EnergyDataBuilder::LaserLines(energy_laser_lines) => rsx! {
            EnergyLaserLineEditor { energy_laser_lines, on_save, readonly }
        },
    }
}
