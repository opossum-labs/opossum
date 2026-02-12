use crate::{
    OPOSSUM_UI_LOGS,
    components::{
        logger::LogResultExt,
        node_editor::inputs::{
            InputParam, IntoInputData, IntoInputDataStrings, format_si_notation,
            input_components::RowedInputs, parse_si_number, parse_unit_input_strict,
        },
    },
};

use dioxus::prelude::*;
use opossum_core::meter;
use opossum_core::spectral_distribution::{LaserLines, SpecDistType};
use strum::EnumIter;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum LaserLinesParam {
    Wavelength,
    RelativeIntensity,
}

impl From<LaserLinesParam> for InputParam {
    fn from(value: LaserLinesParam) -> Self {
        match value {
            LaserLinesParam::Wavelength => Self::SIUnit("Wavelength".into(), "m".into()),
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
                Self::Wavelength => format!("{}", 1054e-9),
                Self::RelativeIntensity => format!("{:.3}", 1.0),
            },
            |laser_line| match self {
                Self::Wavelength => format!("{}", laser_line.0.value),
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
pub fn LaserLineInput(laser_lines: LaserLines, on_save: EventHandler<SpecDistType>) -> Element {
    let inputs = LaserLinesParam::to_input_data_vec(&laser_lines, on_save);
    rsx! {
        form {
            onsubmit: {
                let mut ll = laser_lines;
                move |e: Event<FormData>| {
                    let wvl_opt = e.data().get_first(&inputs[0].id);
                    let rel_int_opt = e.data().get_first(&inputs[1].id);
                    if let (
                        Some(FormValue::Text(wvl_val)),
                        Some(FormValue::Text(rel_int_val)),
                    ) = (wvl_opt.clone(), rel_int_opt.clone()) {
                        if let Ok((num_str, prefix_str)) = parse_unit_input_strict(
                            &wvl_val,
                            "m",
                        ) {
                            if let (Some(wvl), Ok(rel_int)) = (
                                parse_si_number(&num_str, &prefix_str, false),
                                rel_int_val.parse(),
                            ) {
                                match ll.add_lines(vec![(meter!(wvl), rel_int)]) {
                                    Ok(()) => {
                                        on_save.call(SpecDistType::LaserLines(ll.clone()));
                                    }
                                    Err(e) => {
                                        OPOSSUM_UI_LOGS
                                            .write()
                                            .add_log(format!("Error adding laser line: {e}").as_str());
                                    }
                                }
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
            LaserLineList { laser_lines: laser_lines.clone(), on_save }
        }
    }
}

#[component]
fn LaserLineList(laser_lines: LaserLines, on_save: EventHandler<SpecDistType>) -> Element {
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
                            span { {format!("λ: {}m", format_si_notation(line.0.value, false))} }
                            span { {format!("Int: {:.3}", line.1)} }
                            a {
                                class: "text-danger ms-auto",
                                onclick: {
                                    let laser_lines = laser_lines.clone();
                                    move |_| {
                                        let mut laser_lines = laser_lines.clone();
                                        laser_lines.delete_line(i).log_err_with_context("Deleting line failed");
                                        on_save.call(SpecDistType::LaserLines(laser_lines));
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
