#![allow(clippy::derive_partial_eq_without_eq)]
use crate::{
    components::node_editor::{
        accordion::{AccordionItem, LabeledSelect},
        source_editor::{DistInput, DistParam, LightDataBuilderHistory, RowedDistInputs},
    },
    OPOSSUM_UI_LOGS,
};
use dioxus::prelude::*;
use opossum_backend::{
    light_data_builder::LightDataBuilder, nanometer, ray_data_builder::RayDataBuilder, SpecDistType,
};
use strum::IntoEnumIterator;
use uom::si::length::nanometer;

#[component]
pub fn RaySpectralDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let (show, _) = light_data_builder_sig.read().is_rays_is_collimated();
    let rays_spectral_dist = light_data_builder_sig
        .read()
        .get_current_spectral_dist_type();

    if show {
        rsx! {
            div { hidden: !show,
                {
                     rays_spectral_dist.map_or_else(|| rsx! {}, |spectral_dist_type| rsx! {
                             NodeSpectralDistInputs { spectral_dist_type, light_data_builder_sig }
                         })
                }
            }
        }
    } else {
        rsx! {}
    }
}

#[component]
pub fn NodeSpectralDistInputs(
    spectral_dist_type: SpecDistType,
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let dist_params = get_spectral_dist_input_params(&spectral_dist_type, light_data_builder_sig);
    match spectral_dist_type {
        SpecDistType::Gaussian(_) => rsx! {
            RowedDistInputs { dist_params: dist_params }
        },
        SpecDistType::LaserLines(laser_lines) => {
            rsx! {
                form {
                    onsubmit: {
                        move |e: Event<FormData>| {
                            let values = e.data().values();
                            let wvl_opt = values.get(&dist_params[0].id);
                            let rel_int_opt = values.get(&dist_params[1].id);
                            if let (Some(wvl_val), Some(rel_int_val)) = (wvl_opt, rel_int_opt) {
                                if let (Ok(wvl), Ok(rel_int)) = (
                                    wvl_val.as_value().parse::<f64>(),
                                    rel_int_val.as_value().parse::<f64>(),
                                ) {
                                    let mut laser_lines = laser_lines.clone();
                                    if laser_lines.add_lines(vec![(nanometer!(wvl), rel_int)]).is_ok() {
                                        light_data_builder_sig
                                            .with_mut(|ldb| {
                                                ldb.set_spectral_dist_type(
                                                    SpecDistType::LaserLines(laser_lines),
                                                );
                                            });
                                    }
                                } else {
                                    OPOSSUM_UI_LOGS
                                        .write()
                                        .add_log(
                                            format!(
                                                "Could not parse laser line inputs! Wavelength: {wvl_opt:?}. Relative Intensity: {rel_int_opt:?}"

                                            )
                                                .as_str(),
                                        );
                                }
                            } else {
                                OPOSSUM_UI_LOGS
                                    .write()
                                    .add_log(
                                        format!(
                                            "Wrong input inputs for adding laser line! Wavelength: {wvl_opt:?}. Relative Intensity: {rel_int_opt:?}"


                                        )
                                            .as_str(),
                                    );
                            }
                        }
                    },
                    RowedDistInputs { dist_params: dist_params.clone() }
                    input {
                        class: " border-start btn",
                        r#type: "submit",
                        id: "laserlinesubmit",
                        value: "Add laser line",
                    }
                    ul { class: "list-group border-start", id: "laserLineList",
                        for (i , line) in laser_lines.clone().lines().iter().enumerate() {
                            {
                                let class = if i % 2 == 0 {
                                    "list-group-item d-grid text-secondary"
                                } else {
                                    "list-group-item d-grid text-secondary list-group-item-dark"
                                };
                                rsx! {
                                    li { class,
                                        span { {format!("λ: {:.3} nm", line.0.get::<nanometer>())} }
                                        span { {format!("Int: {:.3}", line.1)} }
                                        a {
                                            class: "text-danger ms-auto",
                                            onclick: {
                                                let laser_lines = laser_lines.clone();
                                                move |_| {
                                                    let mut laser_lines = laser_lines.clone();
                                                    println!("deleting line {i}");
                                                    laser_lines.delete_line(i);
                                                    light_data_builder_sig
                                                        .with_mut(|ldb| {
                                                            ldb.set_spectral_dist_type(SpecDistType::LaserLines(laser_lines));
                                                        });
                                                }
                                            },
                                            role: "button",
                                            "🗑︎"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn SpectralDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let accordion_item_content = rsx! {
        RaySpectralDistributionSelector { light_data_builder_sig }
        RaySpectralDistributionEditor { light_data_builder_sig }
    };

    rsx! {
        AccordionItem {
            elements: vec![accordion_item_content],
            header: "Spectral Distribution",
            header_id: "sourceSpectralDistHeading",
            parent_id: "accordionSourceDists",
            content_id: "sourceSpectralDistCollapse",
        }
    }
}

#[component]
pub fn RaySpectralDistributionSelector(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let (show, _) = light_data_builder_sig.read().is_rays_is_collimated();
    let rays_spectral_dist =
        SpecDistSelection::try_from(light_data_builder_sig.read().get_current());

    rays_spectral_dist.map_or_else(
        |_| rsx! {},
        |rsd| {
            rsx! {
                LabeledSelect {
                    id: "selectRaysSpectralDistribution",
                    label: "Rays Spectral Distribution",
                    options: rsd.get_option_elements(),
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
        },
    )
}

impl TryFrom<Option<&LightDataBuilder>> for SpecDistSelection {
    type Error = String;

    fn try_from(value: Option<&LightDataBuilder>) -> Result<Self, Self::Error> {
        match value {
            Some(LightDataBuilder::Geometric(ray_data_builder)) => match ray_data_builder {
                RayDataBuilder::Collimated(collimated_src) => {
                    Ok(Self::new(collimated_src.spect_dist().clone()))
                }
                RayDataBuilder::PointSrc(point_src) => {
                    Ok(Self::new(point_src.spect_dist().clone()))
                }
                RayDataBuilder::Raw(_rays) => Err("not used yet".to_owned()),
                RayDataBuilder::Image { .. } => Err("not used yet".to_owned()),
            },
            _ => Err("Wrong Lightdatabuilder type!".to_owned()),
        }
    }
}

#[derive(Clone, PartialEq)]
struct SpecDistSelection {
    pub spectral_dist: SpecDistType,
    pub laser_lines: bool,
    pub gaussian: bool,
}

impl SpecDistSelection {
    pub fn new(spectral_dist: SpecDistType) -> Self {
        let mut select = Self {
            spectral_dist: spectral_dist.clone(),
            laser_lines: false,
            gaussian: false,
        };

        select.set_dist(spectral_dist);
        select
    }
    pub fn set_dist(&mut self, spectral_dist: SpecDistType) {
        (self.laser_lines, self.gaussian) = match spectral_dist {
            SpecDistType::LaserLines(_) => (true, false),
            SpecDistType::Gaussian(_) => (false, true),
        };

        self.spectral_dist = spectral_dist;
    }

    pub fn get_option_elements(&self) -> Vec<(bool, String)> {
        let mut option_vals = Vec::<(bool, String)>::new();
        for spectral_dist in SpecDistType::iter() {
            option_vals.push(match spectral_dist {
                SpecDistType::LaserLines(_) => (self.laser_lines, spectral_dist.to_string()),
                SpecDistType::Gaussian(_) => (self.gaussian, spectral_dist.to_string()),
            });
        }
        option_vals
    }
}

fn get_spectral_dist_input_params(
    spectral_dist_type: &SpecDistType,
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Vec<DistInput> {
    let mut dist_inputs: Vec<DistInput> = match spectral_dist_type {
        SpecDistType::LaserLines(_) => vec![
            DistInput::new(
                DistParam::WaveLength,
                spectral_dist_type,
                None,
                "1054.".to_string(),
            ),
            DistInput::new(
                DistParam::RelIntensity,
                spectral_dist_type,
                None,
                "1.".to_string(),
            ),
        ],
        SpecDistType::Gaussian(gaussian) => vec![
            DistInput::new(
                DistParam::PointsX,
                spectral_dist_type,
                None,
                format!("{}", gaussian.num_points()),
            ),
            DistInput::new(
                DistParam::CenterX,
                spectral_dist_type,
                None,
                format!("{}", gaussian.mu().get::<nanometer>()),
            ),
            DistInput::new(
                DistParam::WaveLengthStart,
                spectral_dist_type,
                None,
                format!("{}", gaussian.wvl_start().get::<nanometer>()),
            ),
            DistInput::new(
                DistParam::WaveLengthEnd,
                spectral_dist_type,
                None,
                format!("{}", gaussian.wvl_end().get::<nanometer>()),
            ),
            DistInput::new(
                DistParam::Power,
                spectral_dist_type,
                None,
                format!("{}", gaussian.power()),
            ),
            DistInput::new(
                DistParam::FWHM,
                spectral_dist_type,
                None,
                format!("{}", gaussian.fwhm().get::<nanometer>()),
            ),
        ],
    };

    for dist_input in &mut dist_inputs {
        dist_input.callback_opt = use_on_spectral_dist_input_change(
            spectral_dist_type,
            dist_input.dist_param,
            light_data_builder_sig,
        );
    }

    dist_inputs
}

fn use_on_spectral_dist_input_change(
    spectral_dist_type: &SpecDistType,
    param: DistParam,
    mut light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Option<Callback<Event<FormData>>> {
    match *spectral_dist_type {
        SpecDistType::Gaussian(gaussian) => Some(use_callback(move |e: Event<FormData>| {
            let mut gaussian = gaussian;
            if let Ok(value) = e.value().parse::<f64>() {
                match param {
                    DistParam::CenterX => gaussian.set_mu(nanometer!(value)),
                    DistParam::WaveLengthStart => gaussian.set_wvl_start(nanometer!(value)),
                    DistParam::WaveLengthEnd => gaussian.set_wvl_end(nanometer!(value)),
                    DistParam::Power => gaussian.set_power(value),
                    DistParam::FWHM => gaussian.set_fwhm(nanometer!(value)),
                    _ => {}
                }
            } else if let Ok(value) = e.value().parse::<usize>() {
                if DistParam::PointsX == param {
                    gaussian.set_num_points(value);
                }
            }
            light_data_builder_sig
                .with_mut(|ldb| ldb.set_spectral_dist_type(SpecDistType::Gaussian(gaussian)));
        })),
        SpecDistType::LaserLines(_) => None,
    }
}
