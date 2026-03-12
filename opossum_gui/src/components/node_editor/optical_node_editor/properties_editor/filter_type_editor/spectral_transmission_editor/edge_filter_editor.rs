use dioxus::prelude::*;
use opossum_core::prelude::{EdgeFilter, EdgeFilterType, SpectralFilterBuilder, meter};
use strum::{EnumIter, IntoEnumIterator};

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::inputs::{
        InputData, InputParam, IntoInputData, IntoInputDataStrings, input_components::RowedInputs,
        select_options_from_enum_iterator,
    },
};

#[component]
pub fn EdgeFilterEditor(
    edge_filter: EdgeFilter,
    on_spectral_filter_change: EventHandler<SpectralFilterBuilder>,
        readonly: bool
) -> Element {
    let mut edge_filter_sig = use_signal(|| edge_filter.clone());

    let on_edge_filter_change = EventHandler::new(move |new_edge_filter: EdgeFilter| {
        if new_edge_filter != *edge_filter_sig.read() {
            on_spectral_filter_change
                .call(SpectralFilterBuilder::EdgeFilter(new_edge_filter.clone()));
            edge_filter_sig.set(new_edge_filter);
        }
    });

    let mut inputs = Vec::<InputData>::new();
    for param in EdgeFilterParam::iter() {
        if let EdgeFilterParam::FilterType(_) = param {
            inputs.push(
                IntoInputData::<EdgeFilterType, EdgeFilter, EdgeFilter>::to_input_data(
                    &EdgeFilterParam::FilterType(*edge_filter_sig.read().edge_filter_type()),
                    edge_filter_sig.read().clone(),
                    on_edge_filter_change,
                    readonly
                ),
            );
        } else {
            inputs.push(IntoInputData::<f64, EdgeFilter, EdgeFilter>::to_input_data(
                &param,
                edge_filter_sig.read().clone(),
                on_edge_filter_change,
                readonly
            ));
        }
    }

    rsx! {
        RowedInputs { inputs }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum EdgeFilterParam {
    FilterType(EdgeFilterType),
    EdgeWavelength,
    SmoothStepWidth,
    Resolution,
    TransmissionStart,
    TransmissionEnd,
    RangeStart,
    RangeEnd,
}

impl From<EdgeFilterParam> for InputParam {
    fn from(value: EdgeFilterParam) -> Self {
        match value {
            EdgeFilterParam::FilterType(bft) => Self::Selection(
                "Edge filter type".to_string(),
                select_options_from_enum_iterator(&bft, None),
            ),
            EdgeFilterParam::TransmissionStart => Self::F64("Min. transmission".to_string()),
            EdgeFilterParam::TransmissionEnd => Self::F64("Max. transmission".to_string()),
            EdgeFilterParam::EdgeWavelength => Self::SIUnit("Edge λ".to_string(), "m".into()),
            EdgeFilterParam::SmoothStepWidth => Self::SIUnit("Smoothing".to_string(), "m".into()),
            EdgeFilterParam::RangeStart => Self::SIUnit("Start λ".to_string(), "m".into()),
            EdgeFilterParam::RangeEnd => Self::SIUnit("End λ".to_string(), "m".into()),
            EdgeFilterParam::Resolution => Self::SIUnit("Resolution".to_string(), "m".into()),
        }
    }
}

impl IntoInputDataStrings<EdgeFilter> for EdgeFilterParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::FilterType(_) => "FilterType",
            Self::EdgeWavelength => "EdgeWvl",
            Self::TransmissionStart => "TransmissionStart",
            Self::TransmissionEnd => "TransmissionEnd",
            Self::SmoothStepWidth => "SmoothingWidth",
            Self::RangeStart => "StartWvl",
            Self::RangeEnd => "EndWvl",
            Self::Resolution => "Resolution",
        };

        format!("edgeFilterParam{id_str}Input")
    }
    fn create_value_string(&self, obj: &EdgeFilter) -> String {
        match self {
            Self::FilterType(bft) => bft.to_string(),
            Self::EdgeWavelength => format!("{}", obj.edge_wavelength().value),
            Self::SmoothStepWidth => format!("{}", obj.smooth_step_width().map_or(0., |s| s.value)),
            Self::TransmissionStart => format!("{:.3}", obj.transmission_range().start),
            Self::TransmissionEnd => format!("{:.3}", obj.transmission_range().end),
            Self::RangeStart => format!("{}", obj.range().start.value),
            Self::RangeEnd => format!("{}", obj.range().end.value),
            Self::Resolution => format!("{}", obj.resolution().value),
        }
    }
}

impl IntoInputData<f64, EdgeFilter, EdgeFilter> for EdgeFilterParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut EdgeFilter, f64) {
        match self {
            Self::FilterType(_) => move |_: &mut EdgeFilter, _: f64| {},
            Self::EdgeWavelength => move |obj: &mut EdgeFilter, val: f64| {
                obj.set_edge_wavelength(meter!(val)).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid edge wavelength value: {val}"));
                });
            },
            Self::SmoothStepWidth => move |obj: &mut EdgeFilter, val: f64| {
                if val <= 0. {
                    obj.set_smooth_step_width(None).unwrap_or_else(|_| {
                        OPOSSUM_UI_LOGS
                            .write()
                            .add_log(&format!("Invalid smoothing step-width value: {val}"));
                    });
                } else {
                    obj.set_smooth_step_width(Some(meter!(val)))
                        .unwrap_or_else(|_| {
                            OPOSSUM_UI_LOGS
                                .write()
                                .add_log(&format!("Invalid smoothing step-width value: {val}"));
                        });
                }
            },

            Self::TransmissionStart => move |obj: &mut EdgeFilter, val: f64| {
                obj.set_transmission_range_start(val).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid minimum transmission value: {val}"));
                });
            },
            Self::TransmissionEnd => move |obj: &mut EdgeFilter, val: f64| {
                obj.set_transmission_range_end(val).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid maximum transmission value: {val}"));
                });
            },
            Self::RangeStart => move |obj: &mut EdgeFilter, val: f64| {
                obj.set_range_start(meter!(val)).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid range-start value: {val}"));
                });
            },
            Self::RangeEnd => move |obj: &mut EdgeFilter, val: f64| {
                obj.set_range_end(meter!(val)).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid range-end value: {val}"));
                });
            },
            Self::Resolution => move |obj: &mut EdgeFilter, val: f64| {
                obj.set_resolution(meter!(val)).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid edge-filter resolution value: {val}"));
                });
            },
        }
    }
}

impl IntoInputData<EdgeFilterType, EdgeFilter, EdgeFilter> for EdgeFilterParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<EdgeFilterType> {
        let e_value = e.value();
        e_value.parse::<EdgeFilterType>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut EdgeFilter, EdgeFilterType) {
        match self {
            Self::FilterType(_) => {
                move |obj: &mut EdgeFilter, val: EdgeFilterType| obj.set_edge_filter_type(val)
            }
            _ => move |_: &mut EdgeFilter, _: EdgeFilterType| {},
        }
    }
}
