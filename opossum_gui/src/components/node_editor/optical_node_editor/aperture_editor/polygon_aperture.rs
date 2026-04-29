use crate::{
    OPOSSUM_UI_LOGS,
    components::{
        logger::LogResultExt,
        node_editor::inputs::{
            InputParam, IntoInputData, IntoInputDataStrings, format_si_with_base_unit,
            input_components::RowedInputs, parse_si_number, parse_unit_input_strict,
        },
    },
};
use dioxus::prelude::*;
use opossum_core::meter;
use opossum_core::{apertures::PolygonConfig, prelude::ApertureShape};
use strum::EnumIter;

#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
pub enum PolygonConfigParam {
    X,
    Y,
}

impl From<PolygonConfigParam> for InputParam {
    fn from(value: PolygonConfigParam) -> Self {
        match value {
            PolygonConfigParam::X => Self::SIUnit("Pos x".into(), "m".into()),
            PolygonConfigParam::Y => Self::SIUnit("Pos y".into(), "m".into()),
        }
    }
}

impl IntoInputDataStrings<PolygonConfig> for PolygonConfigParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::X => "PosX",
            Self::Y => "PosY",
        };
        format!("polygonConfig{id_str}Input")
    }
    fn create_value_string(&self, obj: &PolygonConfig) -> String {
        obj.points().last().map_or_else(
            || match self {
                Self::X | Self::Y => format!("{}", 0.0),
            },
            |p| match self {
                Self::X => format!("{}", p.x.value),
                Self::Y => format!("{}", p.y.value),
            },
        )
    }
}

impl IntoInputData<f64, PolygonConfig, ApertureShape> for PolygonConfigParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut PolygonConfig, f64) {
        move |_: &mut PolygonConfig, _: f64| {}
    }
}

#[component]
pub fn PolygonApertureInput(
    polygon_config: PolygonConfig,
    on_shape_change: EventHandler<ApertureShape>,
    readonly: bool,
) -> Element {
    let inputs = PolygonConfigParam::to_input_data_vec(&polygon_config, on_shape_change, readonly);
    rsx! {
        form {
            onsubmit: {
                let mut pp = polygon_config;
                move |e: Event<FormData>| {
                    if !readonly {

                        let x_pos_opt = e.data().get_first(&inputs[0].id);
                        let y_pos_opt = e.data().get_first(&inputs[1].id);
                        if let (
                            Some(FormValue::Text(x_pos_val)),
                            Some(FormValue::Text(y_pos_val)),
                        ) = (x_pos_opt.clone(), y_pos_opt.clone()) {
                            if let (

                                Ok((x_num_str, x_prefix_str)),
                                Ok((y_num_str, y_prefix_str)),
                            ) = (
                                parse_unit_input_strict(&x_pos_val, "m"),
                                parse_unit_input_strict(&y_pos_val, "m"),
                            ) {
                                if let (Some(x), Some(y)) = (
                                    parse_si_number(&x_num_str, &x_prefix_str, false),
                                    parse_si_number(&y_num_str, &y_prefix_str, false),
                                ) {
                                    match pp.add_points(&[meter!(x, y)]) {
                                        Ok(()) => {
                                            on_shape_change.call(ApertureShape::BinaryPolygon(pp.clone()));
                                        }
                                        Err(e) => {
                                            OPOSSUM_UI_LOGS
                                                .write()
                                                .add_log(
                                                    format!("Error adding polygon points: {e}").as_str(),
                                                );
                                        }
                                    }
                                }
                            } else {
                                OPOSSUM_UI_LOGS
                                    .write()
                                    .add_log(
                                        format!(
                                            "Could not parse laser line inputs! Wavelength: {x_pos_opt:?}. Relative Intensity: {y_pos_opt:?}",
                                        )
                                            .as_str(),
                                    );
                            }
                        } else {
                            OPOSSUM_UI_LOGS
                                .write()
                                .add_log(
                                    format!(
                                        "Wrong input inputs for adding laser line! Wavelength: {x_pos_opt:?}. Relative Intensity: {y_pos_opt:?}",
                                    )
                                        .as_str(),
                                );
                        }
                    }
                }
            },
            RowedInputs { inputs: inputs.clone() }
            input {
                class: " border-start btn",
                r#type: "submit",
                id: "polygonPointSubmit",
                value: "Add point",
                readonly,
                disabled: readonly,
            }
            PolygonPointList {
                polygon_config: polygon_config.clone(),
                on_shape_change,
                readonly,
            }
        }
    }
}

#[component]
fn PolygonPointList(
    polygon_config: PolygonConfig,
    on_shape_change: EventHandler<ApertureShape>,
    readonly: bool,
) -> Element {
    rsx! {
        ul { class: "list-group border-start", id: "polygonPointList",
            for (i , point) in polygon_config.clone().points().iter().enumerate() {
                {
                    let class = if i % 2 == 0 {
                        "list-group-item d-grid text-secondary"
                    } else {
                        "list-group-item d-grid text-secondary list-group-item-dark"
                    };
                    rsx! {
                        li { class,
                            span { {format!("x: {}", format_si_with_base_unit(point.x.value, "m", false))} }
                            span { {format!("y: {}", format_si_with_base_unit(point.y.value, "m", false))} }
                            a {
                                class: if readonly { "ms-auto text-muted" } else { "text-danger ms-auto" },
                                onclick: {
                                    let polygon_config = polygon_config.clone();
                                    move |_| {
                                        if !readonly {
                                            let mut polygon_config = polygon_config.clone();
                                            polygon_config
                                                .delete_point(i)
                                                .log_err_with_context("Deleting point failed");
                                            on_shape_change.call(ApertureShape::BinaryPolygon(polygon_config));
                                        }
                                    }
                                },
                                role: if readonly { "" } else { "button" },
                                "🗑︎"
                            }
                        }
                    }
                }
            }
        }
    }
}
