use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::inputs::{
        InputParam, IntoInputData, IntoInputDataStrings, input_components::RowedInputs,
    },
};
use dioxus::prelude::*;
use opossum_backend::{LaserLines, SpecDistType, nanometer};
use strum::EnumIter;
use uom::si::length::nanometer;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum LaserLinesParam {
    Wavelength,
    RelativeIntensity,
}

impl From<LaserLinesParam> for InputParam {
    fn from(value: LaserLinesParam) -> Self {
        match value {
            LaserLinesParam::Wavelength => Self::Length("Wavelength in nm".into()),
            LaserLinesParam::RelativeIntensity => Self::F64("Relative intensity".into()),
        }
    }
}

impl IntoInputDataStrings<LaserLines> for LaserLinesParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::Wavelength => "Wavelength",
            Self::RelativeIntensity => "RelativeIntensity",
        };
        format!("laserLines{id_str}Input")
    }
    fn create_value_string(&self, obj: &LaserLines) -> String {
        obj.lines().last().map_or_else(
            || match self {
                Self::Wavelength => format!("{:.3}", 1054),
                Self::RelativeIntensity => format!("{:.3}", 1.0),
            },
            |laser_line| match self {
                Self::Wavelength => format!("{:.3}", laser_line.0.get::<nanometer>()),
                Self::RelativeIntensity => format!("{:.3}", laser_line.1),
            },
        )
    }
}

impl IntoInputData<f64, LaserLines, SpecDistType> for LaserLinesParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut LaserLines, f64) {
        move |_: &mut LaserLines, _: f64| {}
    }
}

#[component]
pub fn LaserLineInput(
    laser_lines: LaserLines,
    spect_dist_type_sig: Signal<SpecDistType>,
) -> Element {
    let inputs = LaserLinesParam::to_input_data_vec(&laser_lines, spect_dist_type_sig);
    rsx! {
        form {
            onsubmit: {
                move |e: Event<FormData>| {
                    let values = e.data().values();
                    let wvl_opt = values.get(&inputs[0].id);
                    let rel_int_opt = values.get(&inputs[1].id);
                    if let (Some(wvl_val), Some(rel_int_val)) = (wvl_opt, rel_int_opt) {
                        if let (Ok(wvl), Ok(rel_int)) = (
                            wvl_val.as_value().parse::<f64>(),
                            rel_int_val.as_value().parse::<f64>(),
                        ) {
                            if let SpecDistType::LaserLines(ll) = &mut *spect_dist_type_sig
                                .write()
                            {
                                ll.add_lines(vec![(nanometer!(wvl), rel_int)])
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
                                        "Could not parse laser line inputs! Wavelength: {wvl_opt:?}. Relative Intensity: {rel_int_opt:?}",
                                    )
                                        .as_str(),
                                );
                        }
                    } else {
                        OPOSSUM_UI_LOGS
                            .write()
                            .add_log(
                                format!(
                                    "Wrong input inputs for adding laser line! Wavelength: {wvl_opt:?}. Relative Intensity: {rel_int_opt:?}",
                                )
                                    .as_str(),
                            );
                    }
                }
            },
            RowedInputs { inputs: inputs.clone() }
            input {
                class: " border-start btn",
                r#type: "submit",
                id: "laserlinesubmit",
                value: "Add laser line",
            }
            LaserLineList { laser_lines, spect_dist_type_sig }
        }
    }
}

#[component]
fn LaserLineList(laser_lines: LaserLines, spect_dist_type_sig: Signal<SpecDistType>) -> Element {
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
                            span { {format!("Int: {:.3}", line.1)} }
                            a {
                                class: "text-danger ms-auto",
                                onclick: {
                                    let laser_lines = laser_lines.clone();
                                    move |_| {
                                        let mut laser_lines = laser_lines.clone();
                                        laser_lines.delete_line(i);
                                        spect_dist_type_sig.set(SpecDistType::LaserLines(laser_lines));
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
