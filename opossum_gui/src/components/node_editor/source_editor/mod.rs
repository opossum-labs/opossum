pub mod energy_distribution;
pub mod light_data_builder_selection;
pub mod position_distribution;
pub mod ray_type_selection;
pub mod spectral_distribution;

pub use energy_distribution::*;
pub use light_data_builder_selection::*;
use opossum_backend::{light_data_builder::LightDataBuilder, Isometry, Proptype};
pub use position_distribution::*;
pub use ray_type_selection::*;
pub use spectral_distribution::*;

use crate::components::node_editor::accordion::AccordionItem;

use super::node_editor_component::NodeChange;
use dioxus::prelude::*;

#[component]
pub fn SourceEditor(
    hidden: bool,
    light_data_builder_opt: Option<Proptype>,
    node_change: Signal<Option<NodeChange>>,
) -> Element {
    let mut light_data_builder_sig = Signal::new(LightDataBuilderHistory::default());

    use_effect(move || {
        node_change.set(Some(NodeChange::Property(
            "light data".to_owned(),
            serde_json::to_value(Proptype::LightDataBuilder(Some(
                light_data_builder_sig.read().get_current().clone(),
            )))
            .unwrap(),
        )))
    });

    use_effect(move || node_change.set(Some(NodeChange::Isometry(Isometry::identity()))));
    use_effect(move || {
        let (ld_builder, key) = match &light_data_builder_opt {
            Some(Proptype::LightDataBuilder(Some(ld)))
                if matches!(ld, LightDataBuilder::Geometric(_)) =>
            {
                (ld.clone(), "Rays")
            }
            Some(Proptype::LightDataBuilder(Some(ld))) => (ld.clone(), "Energy"),
            _ => (LightDataBuilder::default(), "Rays"),
        };
        light_data_builder_sig
            .with_mut(|ldb| ldb.replace_or_insert_and_set_current(key, ld_builder))
    });

    let accordion_item_content = rsx! {
        SourceLightDataBuilderSelector { light_data_builder_sig }
        RayDataBuilderSelector { light_data_builder_sig }
        ReferenceLengthEditor { light_data_builder_sig }
        DistributionEditor { light_data_builder_sig }
    };
    rsx! {
        AccordionItem {
            elements: vec![accordion_item_content],
            header: "Light Source",
            header_id: "sourceHeading",
            parent_id: "accordionNodeConfig",
            content_id: "sourceCollapse",
            hidden,
        }
    }
}

#[component]
pub fn DistributionEditor(light_data_builder_sig: Signal<LightDataBuilderHistory>) -> Element {
    let (is_rays, _) = light_data_builder_sig.read().is_rays_is_collimated();

    rsx! {
        div {
            hidden: !is_rays,
            class: "accordion accordion-borderless bg-dark border-start",
            id: "accordionSourceDists",
            PositionDistributionEditor { light_data_builder_sig }
            EnergyDistributionEditor { light_data_builder_sig }
            SpectralDistributionEditor { light_data_builder_sig }
        }
    }
}
