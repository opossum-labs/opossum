use dioxus::prelude::*;
use opossum_core::prelude::{BandFilter, BandFilterType, SpectralFilterBuilder, meter};
use strum::{EnumIter, IntoEnumIterator};

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::inputs::{
        InputData, InputParam, IntoInputData, IntoInputDataStrings, input_components::RowedInputs,
        select_options_from_enum_iterator,
    },
};

#[component]
pub fn BandFilterEditor(
    band_filter: BandFilter,
    on_spectral_filter_change: EventHandler<SpectralFilterBuilder>,
    readonly: bool,
) -> Element {
    let mut band_filter_sig = use_signal(|| band_filter.clone());

    let on_band_filter_change = EventHandler::new(move |new_band_filter: BandFilter| {
        if new_band_filter != *band_filter_sig.read() {
            on_spectral_filter_change
                .call(SpectralFilterBuilder::BandFilter(new_band_filter.clone()));
            band_filter_sig.set(new_band_filter);
        }
    });

    let mut inputs = Vec::<InputData>::new();
    for param in BandFilterParam::iter() {
        if let BandFilterParam::FilterType(_) = param {
            inputs.push(
                IntoInputData::<BandFilterType, BandFilter, BandFilter>::to_input_data(
                    &BandFilterParam::FilterType(*band_filter_sig.read().band_filter_type()),
                    band_filter_sig.read().clone(),
                    on_band_filter_change,
                    readonly,
                ),
            );
        } else {
            inputs.push(IntoInputData::<f64, BandFilter, BandFilter>::to_input_data(
                &param,
                band_filter_sig.read().clone(),
                on_band_filter_change,
                readonly,
            ));
        }
    }

    rsx! {
        RowedInputs { inputs }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum BandFilterParam {
    FilterType(BandFilterType),
    CenterWavelength,
    Width,
    SmoothStepWidth,
    TransmissionStart,
    TransmissionEnd,
    RangeStart,
    RangeEnd,
    Resolution,
}

impl From<BandFilterParam> for InputParam {
    fn from(value: BandFilterParam) -> Self {
        match value {
            BandFilterParam::FilterType(bft) => Self::Selection(
                "Band filter type".to_string(),
                select_options_from_enum_iterator(&bft, None),
            ),
            BandFilterParam::CenterWavelength => Self::SIUnit("Center λ".to_string(), "m".into()),
            BandFilterParam::Width => Self::SIUnit("FWHM".to_string(), "m".into()),
            BandFilterParam::TransmissionStart => Self::F64("Min. transmission".to_string()),
            BandFilterParam::TransmissionEnd => Self::F64("Max. transmission".to_string()),
            BandFilterParam::SmoothStepWidth => Self::SIUnit("Smoothing".to_string(), "m".into()),
            BandFilterParam::RangeStart => Self::SIUnit("Start λ".to_string(), "m".into()),
            BandFilterParam::RangeEnd => Self::SIUnit("End λ".to_string(), "m".into()),
            BandFilterParam::Resolution => Self::SIUnit("Resolution".to_string(), "m".into()),
        }
    }
}

impl IntoInputDataStrings<BandFilter> for BandFilterParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::FilterType(_) => "FilterType",
            Self::CenterWavelength => "CenterWvl",
            Self::Width => "BandWidth",
            Self::TransmissionStart => "TransmissionStart",
            Self::TransmissionEnd => "TransmissionEnd",
            Self::SmoothStepWidth => "SmoothingWidth",
            Self::RangeStart => "StartWvl",
            Self::RangeEnd => "EndWvl",
            Self::Resolution => "Resolution",
        };

        format!("bandFilterParam{id_str}Input")
    }
    fn create_value_string(&self, obj: &BandFilter) -> String {
        match self {
            Self::FilterType(bft) => bft.to_string(),
            Self::CenterWavelength => format!("{}", obj.center_wavelength().value),
            Self::Width => format!("{}", obj.width().value),
            Self::TransmissionStart => format!("{:.3}", obj.transmission_range().start),
            Self::TransmissionEnd => format!("{:.3}", obj.transmission_range().end),
            Self::SmoothStepWidth => format!("{}", obj.smooth_step_width().map_or(0., |s| s.value)),
            Self::RangeStart => format!("{}", obj.range().start.value),
            Self::RangeEnd => format!("{}", obj.range().end.value),
            Self::Resolution => format!("{}", obj.resolution().value),
        }
    }
}

impl IntoInputData<f64, BandFilter, BandFilter> for BandFilterParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut BandFilter, f64) {
        match self {
            Self::FilterType(_) => move |_: &mut BandFilter, _: f64| {},
            Self::CenterWavelength => move |obj: &mut BandFilter, val: f64| {
                obj.set_center_wavelength(meter!(val)).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid center wavelength value: {val}"));
                });
            },
            Self::Width => move |obj: &mut BandFilter, val: f64| {
                obj.set_width(meter!(val)).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid band-filter width value: {val}"));
                });
            },
            Self::SmoothStepWidth => move |obj: &mut BandFilter, val: f64| {
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

            Self::RangeStart => move |obj: &mut BandFilter, val: f64| {
                obj.set_range_start(meter!(val)).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid range-start value: {val}"));
                });
            },
            Self::RangeEnd => move |obj: &mut BandFilter, val: f64| {
                obj.set_range_end(meter!(val)).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid range-end value: {val}"));
                });
            },
            Self::TransmissionStart => move |obj: &mut BandFilter, val: f64| {
                obj.set_transmission_range_start(val).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid minimum transmission value: {val}"));
                });
            },
            Self::TransmissionEnd => move |obj: &mut BandFilter, val: f64| {
                obj.set_transmission_range_end(val).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid maximum transmission value: {val}"));
                });
            },
            Self::Resolution => move |obj: &mut BandFilter, val: f64| {
                obj.set_resolution(meter!(val)).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid band-filter resolution value: {val}"));
                });
            },
        }
    }
}

impl IntoInputData<BandFilterType, BandFilter, BandFilter> for BandFilterParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<BandFilterType> {
        let e_value = e.value();
        e_value.parse::<BandFilterType>().ok()
    }

    fn setter_from_obj(&self) -> impl FnMut(&mut BandFilter, BandFilterType) {
        match self {
            Self::FilterType(_) => {
                move |obj: &mut BandFilter, val: BandFilterType| obj.set_band_filter_type(val)
            }
            _ => move |_: &mut BandFilter, _: BandFilterType| {},
        }
    }
}
