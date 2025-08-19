use dioxus::prelude::*;
use opossum_backend::{EdgeFilter, EdgeFilterType, SpectralFilterBuilder, nanometer};
use strum::{EnumIter, IntoEnumIterator};
use uom::si::length::nanometer;

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        inputs::{
            InputData, InputParam, IntoInputData, IntoInputDataStrings,
            input_components::RowedInputs, select_options_from_enum_iterator,
        },
        optical_node_editor::properties_editor::use_update_signal_with_reactive_prop,
    },
};

#[component]
pub fn EdgeFilterEditor(
    edge_filter: EdgeFilter,
    spectral_filter_builder_sig: Signal<SpectralFilterBuilder>,
) -> Element {
    let edge_filter_sig = use_signal(|| edge_filter.clone());
    use_update_signal_with_reactive_prop(edge_filter.clone(), edge_filter_sig);

    use_effect({
        let edge_filter = edge_filter;
        move || {
            if edge_filter != *edge_filter_sig.read() {
                spectral_filter_builder_sig.set(SpectralFilterBuilder::EdgeFilter(
                    edge_filter_sig.read().clone(),
                ));
            }
        }
    });

    let mut inputs = Vec::<InputData>::new();
    for param in EdgeFilterParam::iter() {
        if let EdgeFilterParam::FilterType(_) = param {
            inputs.push(
                IntoInputData::<EdgeFilterType, EdgeFilter, EdgeFilter>::to_input_data(
                    &EdgeFilterParam::FilterType(*edge_filter_sig.read().edge_filter_type()),
                    edge_filter_sig.read().clone(),
                    edge_filter_sig,
                ),
            );
        } else {
            inputs.push(IntoInputData::<f64, EdgeFilter, EdgeFilter>::to_input_data(
                &param,
                edge_filter_sig.read().clone(),
                edge_filter_sig,
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
            EdgeFilterParam::TransmissionStart => Self::Length("Min. transmission".to_string()),
            EdgeFilterParam::TransmissionEnd => Self::Length("Max. transmission".to_string()),
            EdgeFilterParam::EdgeWavelength => Self::Length("Edge λ in nm".to_string()),
            EdgeFilterParam::SmoothStepWidth => Self::Length("Smoothing in nm".to_string()),
            EdgeFilterParam::RangeStart => Self::Length("Start λ in nm".to_string()),
            EdgeFilterParam::RangeEnd => Self::Length("End λ in nm".to_string()),
            EdgeFilterParam::Resolution => Self::Length("Resolution in nm".to_string()),
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
            Self::EdgeWavelength => format!("{:.3}", obj.edge_wavelength().get::<nanometer>()),
            Self::SmoothStepWidth => format!(
                "{:.3}",
                obj.smooth_step_width().map_or(0., |s| s.get::<nanometer>())
            ),
            Self::TransmissionStart => format!("{:.3}", obj.transmission_range().start),
            Self::TransmissionEnd => format!("{:.3}", obj.transmission_range().end),
            Self::RangeStart => format!("{:.3}", obj.range().start.get::<nanometer>()),
            Self::RangeEnd => format!("{:.3}", obj.range().end.get::<nanometer>()),
            Self::Resolution => format!("{:.3}", obj.resolution().get::<nanometer>()),
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
                obj.set_edge_wavelength(nanometer!(val))
                    .unwrap_or_else(|_| {
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
                    obj.set_smooth_step_width(Some(nanometer!(val)))
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
                obj.set_range_start(nanometer!(val)).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid range-start value: {val}"));
                });
            },
            Self::RangeEnd => move |obj: &mut EdgeFilter, val: f64| {
                obj.set_range_end(nanometer!(val)).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid range-end value: {val}"));
                });
            },
            Self::Resolution => move |obj: &mut EdgeFilter, val: f64| {
                obj.set_resolution(nanometer!(val)).unwrap_or_else(|_| {
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
