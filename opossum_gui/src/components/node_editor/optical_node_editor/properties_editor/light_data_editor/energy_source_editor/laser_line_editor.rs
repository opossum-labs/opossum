use crate::{
    OPOSSUM_UI_LOGS,
    components::{
        logger::LogResultExt,
        node_editor::inputs::{
            InputData, InputParam, IntoInputData, IntoInputDataStrings,
            input_components::{InputParamLabeledInput, RowedInputs},
        },
    },
};
use dioxus::prelude::*;
use opossum_core::prelude::{EnergyDataBuilder, EnergyLaserLines, joule, nanometer};
use strum::{EnumIter, IntoEnumIterator};
use uom::si::{energy::joule, length::nanometer};

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum EnergyLaserLinesParam {
    Wavelength,
    Energy,
    SpectralResolution,
}

impl From<EnergyLaserLinesParam> for InputParam {
    fn from(value: EnergyLaserLinesParam) -> Self {
        match value {
            EnergyLaserLinesParam::Wavelength => Self::Length("Wavelength in nm".into()),
            EnergyLaserLinesParam::Energy => Self::Energy("Energy in joule".into()),
            EnergyLaserLinesParam::SpectralResolution => Self::Length("Resolution in nm".into()),
        }
    }
}

impl IntoInputDataStrings<EnergyLaserLines> for EnergyLaserLinesParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::Wavelength => "Wavelength",
            Self::SpectralResolution => "SpectralResolution",
            Self::Energy => "Energy",
        };
        format!("energyLaserLines{id_str}Input")
    }
    fn create_value_string(&self, obj: &EnergyLaserLines) -> String {
        match self {
            Self::Wavelength => obj.lines().last().map_or_else(
                || "1054.000".to_string(),
                |ll| format!("{:.3}", ll.0.get::<nanometer>()),
            ),
            Self::Energy => obj.lines().last().map_or_else(
                || "1.000".to_string(),
                |ll| format!("{:.3}", ll.1.get::<joule>()),
            ),
            Self::SpectralResolution => {
                format!("{:.3}", obj.spectral_resolution().get::<nanometer>())
            }
        }
    }
}

impl IntoInputData<f64, EnergyLaserLines, EnergyDataBuilder> for EnergyLaserLinesParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut EnergyLaserLines, f64) {
        if self == &Self::SpectralResolution {
            move |obj: &mut EnergyLaserLines, val: f64| {
                obj.set_spectral_resolution(nanometer!(val))
                    .log_err_with_context("Validation failed in `set_spectral_resolution`");
            }
        } else {
            move |_: &mut EnergyLaserLines, _: f64| {}
        }
    }
}

impl IntoInputData<f64, EnergyLaserLines, EnergyLaserLines> for EnergyLaserLinesParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut EnergyLaserLines, f64) {
        if self == &Self::SpectralResolution {
            move |obj: &mut EnergyLaserLines, val: f64| {
                obj.set_spectral_resolution(nanometer!(val))
                    .log_err_with_context("Validation failed in `set_spectral_resolution`");
            }
        } else {
            move |_: &mut EnergyLaserLines, _: f64| {}
        }
    }
}

#[component]
pub fn EnergyLaserLineEditor(
    energy_laser_lines: EnergyLaserLines,
    energy_data_builder_sig: Signal<EnergyDataBuilder>,
) -> Element {
    let mut form_inputs = Vec::<InputData>::new();
    for elp in EnergyLaserLinesParam::iter() {
        if elp != EnergyLaserLinesParam::SpectralResolution {
            form_inputs.push(
                IntoInputData::<f64, EnergyLaserLines, EnergyDataBuilder>::to_input_data(
                    &elp,
                    energy_laser_lines.clone(),
                    energy_data_builder_sig,
                ),
            );
        }
    }
    let spec_res_input = IntoInputData::<f64, EnergyLaserLines, EnergyDataBuilder>::to_input_data(
        &EnergyLaserLinesParam::SpectralResolution,
        energy_laser_lines.clone(),
        energy_data_builder_sig,
    );
    rsx! {
        form {
            onsubmit: {
                move |e: Event<FormData>| {
                    let wvl_opt = e.data().get_first(&form_inputs[0].id);
                    let energy_opt = e.data().get_first(&form_inputs[1].id);
                    if let (Some(FormValue::Text(wvl_val)), Some(FormValue::Text(energy_val))) = (
                        wvl_opt.clone(),
                        energy_opt.clone(),
                    ) {
                        if let (Ok(wvl), Ok(energy)) = (wvl_val.parse(), energy_val.parse()) {
                            if let EnergyDataBuilder::LaserLines(ll) = &mut *energy_data_builder_sig
                                .write()
                            {
                                ll.add_lines(vec![(nanometer!(wvl), joule!(energy))])
                                    .unwrap_or_else(|e| {
                                        OPOSSUM_UI_LOGS
                                            .write()
                                            .add_log(format!("Error adding laser line: {e}").as_str());
                                    });
                            }
                        } else {
                            OPOSSUM_UI_LOGS
                                .write()
                                .add_log(
                                    format!(
                                        "Could not parse laser line inputs! Wavelength: {wvl_opt:?}. Energy: {energy_opt:?}",
                                    )
                                        .as_str(),
                                );
                        }
                    } else {
                        OPOSSUM_UI_LOGS
                            .write()
                            .add_log(
                                format!(
                                    "Wrong input inputs for adding laser line! Wavelength: {wvl_opt:?}. Energy: {energy_opt:?}",
                                )
                                    .as_str(),
                            );
                    }
                }
            },
            RowedInputs { inputs: form_inputs.clone() }
            input {
                class: " border-start btn",
                r#type: "submit",
                id: "energylaserlinesubmit",
                value: "Add laser line",
            }
            LaserLineList { laser_lines: energy_laser_lines, energy_data_builder_sig }
        }
        InputParamLabeledInput { input_data: spec_res_input }
    }
}

#[component]
fn LaserLineList(
    laser_lines: EnergyLaserLines,
    energy_data_builder_sig: Signal<EnergyDataBuilder>,
) -> Element {
    rsx! {
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
                            span { {format!("E: {:.3} J", line.1.get::<joule>())} }
                            a {
                                class: "text-danger ms-auto",
                                onclick: {
                                    let laser_lines = laser_lines.clone();
                                    move |_| {
                                        let mut laser_lines = laser_lines.clone();
                                        if laser_lines.delete_line(i).is_ok() {
                                            energy_data_builder_sig.set(EnergyDataBuilder::LaserLines(laser_lines));
                                        }
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
