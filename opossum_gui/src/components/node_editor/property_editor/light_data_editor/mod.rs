#![allow(clippy::derive_partial_eq_without_eq)]
pub mod energy_distribution_editor;
pub mod light_data_builder_selection;
pub mod position_distribution_editor;
pub mod ray_type_selection;
pub mod spectral_distribution_editor;

pub use position_distribution_editor::*;
pub use ray_type_selection::*;
pub use spectral_distribution_editor::*;

pub use light_data_builder_selection::*;
use opossum_backend::{light_data_builder::LightDataBuilder, Proptype};

use crate::components::node_editor::{
    accordion::AccordionItem,
    property_editor::light_data_editor::energy_distribution_editor::EnergyDistributionEditor,
};

use dioxus::prelude::*;

#[component]
pub fn LightDataEditor(
    light_data_builder_sig: Signal<LightDataBuilder>,
    prop_type_sig: Signal<Proptype>,
) -> Element {
    use_effect(move || {
        prop_type_sig.set(Proptype::LightDataBuilder(Some(
            light_data_builder_sig.read().clone(),
        )));
    });

    let accordion_item_content = rsx! {
        SourceLightDataBuilderSelector { light_data_builder_sig }
        RayDataBuilderSelector { light_data_builder_sig }
        ReferenceLengthEditor { light_data_builder_sig }
        DistributionEditor { light_data_builder_sig }
        ImageSourceEditor { light_data_builder_sig }
    };
    rsx! {
        div {
            class: "accordion accordion-borderless bg-dark border-start",
            id: "accordionLightDataConfig",
            AccordionItem {
                elements: vec![accordion_item_content],
                header: "Light definition",
                header_id: "sourceHeading",
                parent_id: "accordionLightDataConfig",
                content_id: "sourceCollapse",
            }
        }
    }
}

#[component]
pub fn DistributionEditor(light_data_builder_sig: Signal<LightDataBuilder>) -> Element {
    let pos_dist_type_sig = use_signal(move || {
        light_data_builder_sig
            .read()
            .get_position_distribution_type()
            .unwrap_or_default()
    });
    let energy_dist_type_sig = use_signal(move || {
        light_data_builder_sig
            .read()
            .get_energy_distribution_type()
            .unwrap_or_default()
    });
    let spect_dist_type_sig = use_signal(move || {
        light_data_builder_sig
            .read()
            .get_spectral_distribution_type()
            .unwrap_or_default()
    });

    use_effect(move || {
        if let LightDataBuilder::Geometric(rdb) = &mut *light_data_builder_sig.write() {
            rdb.set_pos_dist(*pos_dist_type_sig.read());
        }
    });

    use_effect(move || {
        if let LightDataBuilder::Geometric(rdb) = &mut *light_data_builder_sig.write() {
            rdb.set_energy_dist(*energy_dist_type_sig.read());
        }
    });

    use_effect(move || {
        if let LightDataBuilder::Geometric(rdb) = &mut *light_data_builder_sig.write() {
            rdb.set_spectral_dist(spect_dist_type_sig.read().clone());
        }
    });

    rsx! {
        div {
            class: "accordion accordion-borderless bg-dark border-start",
            id: "accordionSourceDists",
            PositionDistributionEditor { pos_dist_type_sig }
            EnergyDistributionEditor { energy_dist_type_sig }
            SpectralDistributionEditor { spect_dist_type_sig }
        }
    }
}
