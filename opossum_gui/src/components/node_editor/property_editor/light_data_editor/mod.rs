#![allow(clippy::derive_partial_eq_without_eq)]
pub mod energy_distribution;
pub mod light_data_builder_selection;
pub mod position_distribution;
pub mod ray_type_selection;
pub mod spectral_distribution;

pub use energy_distribution::*;
pub use light_data_builder_selection::*;
use opossum_backend::{light_data_builder::LightDataBuilder, Proptype};
pub use position_distribution::*;
pub use ray_type_selection::*;
pub use spectral_distribution::*;

use crate::components::node_editor::accordion::AccordionItem;

use dioxus::prelude::*;

#[component]
pub fn LightDataEditor(
    light_data_builder_opt: Option<LightDataBuilder>,
    prop_type_sig: Signal<Proptype>,
) -> Element {
    let mut light_data_builder_sig = Signal::new(LightDataBuilderHistory::default());

    use_effect(move || {
        prop_type_sig.set(Proptype::LightDataBuilder(
            light_data_builder_sig.read().get_current().cloned(),
        ));
    });

    use_effect(move || {
        let (ld_builder, key) = match &light_data_builder_opt {
            Some(ld) if matches!(ld, LightDataBuilder::Geometric(_)) => (ld.clone(), "Rays"),
            Some(ld) => (ld.clone(), "Energy"),
            _ => (LightDataBuilder::default(), "Rays"),
        };
        light_data_builder_sig
            .with_mut(|ldb| ldb.replace_or_insert_and_set_current(key, ld_builder));
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
pub fn DistributionEditor(light_data_builder_sig: Signal<LightDataBuilderHistory>) -> Element {
    let (is_rays, is_not_image) = light_data_builder_sig.read().is_rays_is_not_image();

    if is_rays && is_not_image {
        rsx! {
            div {
                class: "accordion accordion-borderless bg-dark border-start",
                id: "accordionSourceDists",
                PositionDistributionEditor { light_data_builder_sig }
                EnergyDistributionEditor { light_data_builder_sig }
                SpectralDistributionEditor { light_data_builder_sig }
            }
        }
    } else {
        rsx! {}
    }
}
