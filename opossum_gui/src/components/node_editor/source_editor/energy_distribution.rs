#![allow(clippy::derive_partial_eq_without_eq)]
use crate::{
    components::node_editor::{
        accordion::{AccordionItem, LabeledSelect},
        source_editor::{
            CallbackWrapper, DistInput, DistParam, LightDataBuilderHistory, RowedDistInputs,
        },
    },
    OPOSSUM_UI_LOGS,
};
use dioxus::prelude::*;
use opossum_backend::{
    degree, joule, light_data_builder::LightDataBuilder, millimeter,
    ray_data_builder::RayDataBuilder, EnergyDistType,
};
use strum::IntoEnumIterator;
use uom::si::{angle::degree, energy::joule, length::millimeter};

#[component]
pub fn RayEnergyDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let (show, _) = light_data_builder_sig.read().is_rays_is_collimated();
    let rays_energy_dist = light_data_builder_sig.read().get_current_energy_dist_type();

    rsx! {
        div { hidden: !show,
            {
                rays_energy_dist
                    .map_or_else(
                        || rsx! {},
                        |energy_dist_type| rsx! {
                            NodeEnergyDistInputs { energy_dist_type, light_data_builder_sig }
                        },
                    )
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
    rsx! {
        RowedDistInputs { dist_params }
    }
}

#[component]
pub fn EnergyDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let accordion_item_content = rsx! {
        RayEnergyDistributionSelector { light_data_builder_sig }
        RayEnergyDistributionEditor { light_data_builder_sig }
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
    let rays_energy_dist =
        EnergyDistSelection::try_from(light_data_builder_sig.read().get_current());

    rays_energy_dist.map_or_else(
        |_| rsx! {},
        |red| {
            rsx! {
                LabeledSelect {
                    id: "selectRaysEnergyDistribution",
                    label: "Rays Energy Distribution",
                    options: red.get_option_elements(),
                    onchange: move |e: Event<FormData>| {
                        light_data_builder_sig
                            .with_mut(|ldb| {
                                let value = e.value();
                                ldb.set_current_or_default(value.as_str());
                            });
                    },
                }
            }
        },
    )
}

impl TryFrom<Option<&LightDataBuilder>> for EnergyDistSelection {
    type Error = String;

    fn try_from(value: Option<&LightDataBuilder>) -> Result<Self, Self::Error> {
        match value {
            Some(LightDataBuilder::Geometric(ray_data_builder)) => match ray_data_builder {
                RayDataBuilder::Collimated(collimated_src) => {
                    Ok(Self::new(*collimated_src.energy_dist()))
                }
                RayDataBuilder::PointSrc(point_src) => Ok(Self::new(*point_src.energy_dist())),
                RayDataBuilder::Raw(_rays) => Err("not used yet".to_owned()),
                RayDataBuilder::Image { .. } => Err("not used yet".to_owned()),
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
    pub const fn new(energy_dist: EnergyDistType) -> Self {
        let mut select = Self {
            energy_dist,
            uniform: false,
            gaussian: false,
        };

        select.set_dist(energy_dist);
        select
    }
    pub const fn set_dist(&mut self, energy_dist: EnergyDistType) {
        (self.uniform, self.gaussian) = match energy_dist {
            EnergyDistType::Uniform(_) => (true, false),
            EnergyDistType::General2DGaussian(_) => (false, true),
        };

        self.energy_dist = energy_dist;
    }

    pub fn get_option_elements(&self) -> Vec<(bool, String)> {
        let mut option_vals = Vec::<(bool, String)>::new();
        for energy_dist in EnergyDistType::iter() {
            option_vals.push(match energy_dist {
                EnergyDistType::Uniform(_) => (self.uniform, energy_dist.to_string()),
                EnergyDistType::General2DGaussian(_) => (self.gaussian, energy_dist.to_string()),
            });
        }
        option_vals
    }
}

fn get_energy_dist_input_params(
    energy_dist_type: EnergyDistType,
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Vec<DistInput> {
    let dist_inputs: Vec<DistInput> = match energy_dist_type {
        EnergyDistType::Uniform(uniform) => vec![DistInput::new(
            DistParam::Energy,
            &energy_dist_type,
            on_energy_dist_input_change(
                energy_dist_type,
                DistParam::Energy,
                light_data_builder_sig,
            ),
            format!("{}", uniform.energy().get::<joule>()),
        )],
        EnergyDistType::General2DGaussian(gaussian) => vec![
            DistInput::new(
                DistParam::CenterX,
                &energy_dist_type,
                on_energy_dist_input_change(
                    energy_dist_type,
                    DistParam::CenterX,
                    light_data_builder_sig,
                ),
                format!("{}", gaussian.center().x.get::<millimeter>()),
            ),
            DistInput::new(
                DistParam::CenterY,
                &energy_dist_type,
                on_energy_dist_input_change(
                    energy_dist_type,
                    DistParam::CenterY,
                    light_data_builder_sig,
                ),
                format!("{}", gaussian.center().y.get::<millimeter>()),
            ),
            DistInput::new(
                DistParam::LengthX,
                &energy_dist_type,
                on_energy_dist_input_change(
                    energy_dist_type,
                    DistParam::LengthX,
                    light_data_builder_sig,
                ),
                format!("{}", gaussian.sigma().x.get::<millimeter>()),
            ),
            DistInput::new(
                DistParam::LengthY,
                &energy_dist_type,
                on_energy_dist_input_change(
                    energy_dist_type,
                    DistParam::LengthY,
                    light_data_builder_sig,
                ),
                format!("{}", gaussian.sigma().y.get::<millimeter>()),
            ),
            DistInput::new(
                DistParam::Energy,
                &energy_dist_type,
                on_energy_dist_input_change(
                    energy_dist_type,
                    DistParam::Energy,
                    light_data_builder_sig,
                ),
                format!("{}", gaussian.energy().get::<joule>()),
            ),
            DistInput::new(
                DistParam::Angle,
                &energy_dist_type,
                on_energy_dist_input_change(
                    energy_dist_type,
                    DistParam::Angle,
                    light_data_builder_sig,
                ),
                format!("{}", gaussian.theta().get::<degree>()),
            ),
            DistInput::new(
                DistParam::Power,
                &energy_dist_type,
                on_energy_dist_input_change(
                    energy_dist_type,
                    DistParam::Power,
                    light_data_builder_sig,
                ),
                format!("{}", gaussian.power()),
            ),
            DistInput::new(
                DistParam::Rectangular,
                &energy_dist_type,
                on_energy_dist_input_change(
                    energy_dist_type,
                    DistParam::Rectangular,
                    light_data_builder_sig,
                ),
                format!("{}", gaussian.rectangular()),
            ),
        ],
    };

    dist_inputs
}

fn on_energy_dist_input_change(
    mut energy_dist_type: EnergyDistType,
    param: DistParam,
    mut light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        let value = e.value();
        if let Ok(value) = value.parse::<f64>() {
            match &mut energy_dist_type {
                EnergyDistType::Uniform(uniform) => {
                    if param == DistParam::Energy {
                        uniform
                            .set_energy(joule!(value))
                            .unwrap_or_else(|e| OPOSSUM_UI_LOGS.write().add_log(&format!("{e}")));
                    }
                }
                EnergyDistType::General2DGaussian(gaussian) => match param {
                    DistParam::CenterX => gaussian.set_center_x(millimeter!(value)),
                    DistParam::CenterY => gaussian.set_center_y(millimeter!(value)),
                    DistParam::LengthX => gaussian.set_sigma_x(millimeter!(value)),
                    DistParam::LengthY => gaussian.set_sigma_y(millimeter!(value)),
                    DistParam::Energy => gaussian
                        .set_energy(joule!(value))
                        .unwrap_or_else(|e| OPOSSUM_UI_LOGS.write().add_log(&format!("{e}"))),
                    DistParam::Angle => gaussian.set_theta(degree!(value)),
                    DistParam::Power => gaussian.set_power(value),
                    _ => {}
                },
            }
        } else if let Ok(value) = value.parse::<bool>() {
            if let EnergyDistType::General2DGaussian(gaussian) = &mut energy_dist_type {
                if param == DistParam::Rectangular {
                    gaussian.set_rectangular(value);
                }
            }
        } else {
            OPOSSUM_UI_LOGS
                .write()
                .add_log("Unable to parse passed value, please check input parameters!");
        }
        light_data_builder_sig.with_mut(|ldb| ldb.set_energy_dist_type(energy_dist_type));
    })
}
