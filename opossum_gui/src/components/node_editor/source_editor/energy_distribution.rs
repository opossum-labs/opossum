use crate::{components::node_editor::{
    accordion::{AccordionItem, LabeledSelect}, source_editor::{DistInput, DistParam, LightDataBuilderHistory, RowedDistInputs},
}, OPOSSUM_UI_LOGS};
use dioxus::prelude::*;
use opossum_backend::{light_data_builder::LightDataBuilder, ray_data_builder::RayDataBuilder, EnergyDistType, joule, millimeter, degree};
use strum_macros::EnumIter;
use strum::IntoEnumIterator;
use uom::si::{angle::degree, energy::joule, length::millimeter};



#[component]
pub fn RayEnergyDistributionEditor(light_data_builder_sig: Signal<LightDataBuilderHistory>) -> Element {
    let (show, _) = light_data_builder_sig.read().is_rays_is_collimated();
    let rays_energy_dist = light_data_builder_sig
        .read()
        .get_current_energy_dist_type()
        .clone();

    rsx! {
        div { hidden: !show,
            {
                if let Some(energy_dist_type) = rays_energy_dist {
                    rsx! {
                        NodeEnergyDistInputs { energy_dist_type, light_data_builder_sig }
                    }
                } else {
                    rsx! {}
                }
            }
        }
    }
}


#[component]
pub fn NodeEnergyDistInputs(
    energy_dist_type: EnergyDistType,
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let dist_params = get_energy_dist_input_params(energy_dist_type, light_data_builder_sig);
    rsx!{
        RowedDistInputs {dist_params}
    }
}

#[component]
pub fn EnergyDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let accordion_item_content = rsx! {
        RayEnergyDistributionSelector { light_data_builder_sig }
        RayEnergyDistributionEditor{ light_data_builder_sig }
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



fn get_energy_dist_input_params(energy_dist_type: EnergyDistType, light_data_builder_sig: Signal<LightDataBuilderHistory>) -> Vec<DistInput> {    
    let mut dist_inputs :Vec<DistInput> = match energy_dist_type {
        EnergyDistType::Uniform(uniform) => vec![
            DistInput::new(DistParam::Energy, &energy_dist_type, None, format!("{}", uniform.energy().get::<joule>())),
        ],
        EnergyDistType::General2DGaussian(gaussian) => vec![
        DistInput::new(DistParam::CenterX, &energy_dist_type, None, format!("{}", gaussian.center().x.get::<millimeter>())),
        DistInput::new(DistParam::CenterY, &energy_dist_type, None, format!("{}", gaussian.center().y.get::<millimeter>())),
        DistInput::new(DistParam::LengthX, &energy_dist_type, None, format!("{}", gaussian.sigma().x.get::<millimeter>())),
        DistInput::new(DistParam::LengthY, &energy_dist_type, None, format!("{}", gaussian.sigma().y.get::<millimeter>())),
        DistInput::new(DistParam::Energy, &energy_dist_type, None, format!("{}", gaussian.energy().get::<joule>())),
        DistInput::new(DistParam::Angle, &energy_dist_type, None, format!("{}", gaussian.theta().get::<degree>())),
        DistInput::new(DistParam::Power, &energy_dist_type, None, format!("{}", gaussian.power())),
        DistInput::new(DistParam::Rectangular, &energy_dist_type, None, format!("{}", gaussian.rectangular())),
        ]
    };

    for (dist_input) in &mut dist_inputs{
        dist_input.callback_opt = use_on_energy_dist_input_change(
                                        energy_dist_type,
                                        dist_input.dist_param.clone(),
                                        light_data_builder_sig.clone(),
                                    );
    }   

    dist_inputs
}


fn use_on_energy_dist_input_change(
    mut energy_dist_type: EnergyDistType,
    param: DistParam,
    mut light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Option<Callback<Event<FormData>>> {
    Some(use_callback(move |e: Event<FormData>| {
        let value = e.value();
        if let Ok(value) = value.parse::<f64>() {
            match &mut energy_dist_type {
                EnergyDistType::Uniform(uniform) => match param {
                    DistParam::Energy => uniform.set_energy(joule!(value)).unwrap_or_else(|e| OPOSSUM_UI_LOGS.write().add_log(&format!("{e}"))),
                    _ => {}
                },
                EnergyDistType::General2DGaussian(gaussian) => match param {
                    DistParam::CenterX => gaussian.set_center_x(millimeter!(value)),
                    DistParam::CenterY => gaussian.set_center_y(millimeter!(value)),
                    DistParam::LengthX => gaussian.set_sigma_x(millimeter!(value)),
                    DistParam::LengthY => gaussian.set_sigma_y(millimeter!(value)),
                    DistParam::Energy => gaussian.set_energy(joule!(value)).unwrap_or_else(|e| OPOSSUM_UI_LOGS.write().add_log(&format!("{e}"))),
                    DistParam::Angle => gaussian.set_theta(degree!(value)),
                    DistParam::Power => gaussian.set_power(value),
                    _ => {}
                },
            }
            light_data_builder_sig.with_mut(|ldb| ldb.set_energy_dist_type(energy_dist_type))
        }
        else if let Ok(value) = value.parse::<bool>(){
            match &mut energy_dist_type {
                EnergyDistType::General2DGaussian(gaussian) => match param {
                    DistParam::Rectangular => gaussian.set_rectangular(value),
                    _ => {}
                },
                _ => {}
            }
            light_data_builder_sig.with_mut(|ldb| ldb.set_energy_dist_type(energy_dist_type))
        }
    }))
}