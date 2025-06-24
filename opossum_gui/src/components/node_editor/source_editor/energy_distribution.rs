use crate::components::node_editor::{
    accordion::{AccordionItem, LabeledSelect}, source_editor::LightDataBuilderHistory,
};
use dioxus::prelude::*;
use opossum_backend::{light_data_builder::LightDataBuilder, ray_data_builder::RayDataBuilder, EnergyDistType};
use strum_macros::EnumIter;
use strum::IntoEnumIterator;


#[component]
pub fn EnergyDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let accordion_item_content = rsx! {
        RayEnergyDistributionSelector { light_data_builder_sig }
    };

    rsx! {
        AccordionItem {
            elements: vec![accordion_item_content],
            header: "Energy Distribution",
            header_id: "sourceEnergyDistHeading",
            parent_id: "accordionSourceDists",
            content_id: "sourceEnergyDistCollapse",
        }
    }
}

#[component]
pub fn RayEnergyDistributionSelector(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let (show, _) = light_data_builder_sig.read().is_rays_is_collimated();
    let rays_energy_dist =
        EnergyDistSelection::try_from(light_data_builder_sig.read().get_current().clone());

    if let Ok(red) = rays_energy_dist {
        rsx! {
            LabeledSelect {
                id: "selectRaysEnergyDistribution",
                label: "Rays Energy Distribution",
                options: red.get_option_elements(),
                hidden: !show,
                onchange: move |e: Event<FormData>| {
                    light_data_builder_sig
                        .with_mut(|ldb| {
                            let value = e.value();
                            ldb.set_current_or_default(value.as_str());
                        });
                },
            }
        }
    } else {
        rsx! {}
    }
}



impl TryFrom<LightDataBuilder> for EnergyDistSelection {
    type Error = String;

    fn try_from(value: LightDataBuilder) -> Result<Self, Self::Error> {
        match value {
            LightDataBuilder::Geometric(ray_data_builder) => match ray_data_builder {
                RayDataBuilder::Collimated(collimated_src) => {
                    Ok(Self::new(collimated_src.energy_dist().clone()))
                }
                RayDataBuilder::PointSrc(point_src) => Ok(Self::new(point_src.energy_dist().clone())),
                RayDataBuilder::Raw(rays) => Err("not used yet".to_owned()),
                RayDataBuilder::Image {
                    file_path,
                    pixel_size,
                    total_energy,
                    wave_length,
                    cone_angle,
                } => Err("not used yet".to_owned()),
            },
            _ => Err("Wrong Lightdatabuilder type!".to_owned()),
        }
    }
}



#[derive(Clone, PartialEq)]
struct EnergyDistSelection {
    pub energy_dist: EnergyDistType,
    pub uniform: bool,
    pub gaussian: bool,
}

impl EnergyDistSelection {
    pub fn new(energy_dist: EnergyDistType) -> Self {
        let mut select = Self {
            energy_dist: energy_dist.clone(),
            uniform: false,
            gaussian: false,

        };

        select.set_dist(energy_dist);
        select
    }
    pub fn set_dist(&mut self, energy_dist: EnergyDistType) {
        (
            self.uniform,
            self.gaussian
        ) = match energy_dist {
            EnergyDistType::Uniform(_) => (true, false),
            EnergyDistType::General2DGaussian(_) =>   (false, true),
        };

        self.energy_dist = energy_dist;
    }

    pub fn get_option_elements(&self) -> Vec<(bool, String)> {
        let mut option_vals = Vec::<(bool, String)>::new();
        for energy_dist in EnergyDistType::iter() {
            option_vals.push(match energy_dist {
                EnergyDistType::Uniform(_) => (self.uniform, energy_dist.to_string()),
                EnergyDistType::General2DGaussian(_) => (self.gaussian, energy_dist.to_string()),
            })
        }
        option_vals
    }
}