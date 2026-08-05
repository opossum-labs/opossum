#![allow(clippy::derive_partial_eq_without_eq)]
mod gaussian_editor;
mod uniform_editor;

use crate::components::node_editor::{
    accordion::AccordionItem,
    hooks::use_synced_signal,
    inputs::{
        InputData, IntoInputData,
        input_components::{LabeledSelect, RowedInputs},
        select_options_from_enum_iterator,
    },
};
use dioxus::prelude::*;
use gaussian_editor::get_general_2d_gaussian_input_params;
use opossum_core::{
    distributions::energy::EnergyDistType, prelude::RayDataSource,
    utils::default_from_name::DefaultFromName,
};
use uniform_editor::UniformParam;

#[component]
pub fn RayEnergyDistributionEditor(
    energy_dist_type_sig: ReadSignal<EnergyDistType>,
    on_save: EventHandler<EnergyDistType>,
    readonly: bool,
) -> Element {
    let inputs: Vec<InputData> =
        get_energy_dist_input_data(energy_dist_type_sig, on_save, readonly);
    rsx! {
        RowedInputs { inputs }
    }
}

#[component]
pub fn EnergyDistributionEditor(
    energy_dist_type: EnergyDistType,
    ray_data_builder_sig: ReadSignal<RayDataSource>,
    on_save: EventHandler<RayDataSource>,
    readonly: bool,
) -> Element {
    let mut energy_dist_type_sig = use_synced_signal(energy_dist_type);

    let on_energy_dist_save = EventHandler::new(move |new_energy_dist_type: EnergyDistType| {
        energy_dist_type_sig.set(new_energy_dist_type);
        let mut ray_data_builder = ray_data_builder_sig.read().clone();
        ray_data_builder.set_energy_dist(*energy_dist_type_sig.read());
        on_save.call(ray_data_builder);
    });

    let accordion_item_content = rsx! {
        RayEnergyDistributionSelector {
            energy_dist_type_sig,
            on_save: on_energy_dist_save,
            readonly,
        }
        RayEnergyDistributionEditor {
            energy_dist_type_sig,
            on_save: on_energy_dist_save,
            readonly,
        }
    };

    rsx! {
        AccordionItem {
            elements: vec![accordion_item_content],
            header: "Energy Distribution",
            header_id: "sourceEnergyDistHeading",
            parent_id: "accordionSourceDists",
            content_id: "sourceEnergyDistCollapse",
            level: 2,
        }
    }
}

#[component]
pub fn RayEnergyDistributionSelector(
    energy_dist_type_sig: ReadSignal<EnergyDistType>,
    on_save: EventHandler<EnergyDistType>,
    readonly: bool,
) -> Element {
    rsx! {
        LabeledSelect {
            id: "selectRaysEnergyDistribution",
            label: "Rays Energy Distribution",
            options: select_options_from_enum_iterator(&*energy_dist_type_sig.read(), None),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(edt) = EnergyDistType::default_from_name(val.as_str()) {
                    on_save.call(edt);
                }
            },
        }
    }
}

fn get_energy_dist_input_data(
    energy_dist_type_sig: ReadSignal<EnergyDistType>,
    on_save: EventHandler<EnergyDistType>,
    readonly: bool,
) -> Vec<InputData> {
    match &*energy_dist_type_sig.read() {
        EnergyDistType::Uniform(u) => UniformParam::to_input_data_vec(u, on_save, readonly),
        EnergyDistType::General2DGaussian(g) => {
            get_general_2d_gaussian_input_params(g, on_save, readonly)
        }
    }
}
