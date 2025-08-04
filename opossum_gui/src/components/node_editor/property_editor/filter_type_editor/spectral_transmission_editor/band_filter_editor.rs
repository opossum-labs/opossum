use dioxus::prelude::*;
use opossum_backend::{BandFilter, BandFilterType, SpectralFilterBuilder, nanometer};
use strum::{EnumIter, IntoEnumIterator};
use uom::si::length::nanometer;

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
    spectral_filter_builder_sig: Signal<SpectralFilterBuilder>,
) -> Element {
    let band_filter_sig = use_signal(|| band_filter.clone());

    use_effect({
        let band_filter = band_filter;
        move || {
            if band_filter != *band_filter_sig.read() {
                spectral_filter_builder_sig.set(SpectralFilterBuilder::BandFilter(
                    band_filter_sig.read().clone(),
                ));
            }
        }
    });

    let mut inputs = Vec::<InputData>::new();
    for param in BandFilterParam::iter() {
        if let BandFilterParam::FilterType(_) = param {
            inputs.push(
                IntoInputData::<BandFilterType, BandFilter, BandFilter>::to_input_data(
                    &BandFilterParam::FilterType(*band_filter_sig.read().band_filter_type()),
                    band_filter_sig.read().clone(),
                    band_filter_sig,
                ),
            );
        } else {
            inputs.push(IntoInputData::<f64, BandFilter, BandFilter>::to_input_data(
                &param,
                band_filter_sig.read().clone(),
                band_filter_sig,
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
            BandFilterParam::CenterWavelength => Self::Length("Center λ in nm".to_string()),
            BandFilterParam::Width => Self::Length("FWHM in nm".to_string()),
            BandFilterParam::SmoothStepWidth => Self::Length("Smoothing width in nm".to_string()),
            BandFilterParam::RangeStart => Self::Length("Start λ in nm".to_string()),
            BandFilterParam::RangeEnd => Self::Length("End λ in nm".to_string()),
            BandFilterParam::Resolution => Self::Length("Resolution in nm".to_string()),
        }
    }
}

impl IntoInputDataStrings<BandFilter> for BandFilterParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::FilterType(_) => "FilterType",
            Self::CenterWavelength => "CenterWvl",
            Self::Width => "BandWidth",
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
            Self::CenterWavelength => format!("{:.3}", obj.center_wavelength().get::<nanometer>()),
            Self::Width => format!("{:.3}", obj.width().get::<nanometer>()),
            Self::SmoothStepWidth => format!(
                "{:.3}",
                obj.smooth_step_width().map_or(0., |s| s.get::<nanometer>())
            ),
            Self::RangeStart => format!("{:.3}", obj.range().start.get::<nanometer>()),
            Self::RangeEnd => format!("{:.3}", obj.range().end.get::<nanometer>()),
            Self::Resolution => format!("{:.3}", obj.resolution().get::<nanometer>()),
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
                obj.set_center_wavelength(nanometer!(val))
                    .unwrap_or_else(|_| {
                        OPOSSUM_UI_LOGS
                            .write()
                            .add_log(&format!("Invalid center wavelength value: {val}"));
                    });
            },
            Self::Width => move |obj: &mut BandFilter, val: f64| {
                obj.set_width(nanometer!(val)).unwrap_or_else(|_| {
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
                    obj.set_smooth_step_width(Some(nanometer!(val)))
                        .unwrap_or_else(|_| {
                            OPOSSUM_UI_LOGS
                                .write()
                                .add_log(&format!("Invalid smoothing step-width value: {val}"));
                        });
                }
            },

            Self::RangeStart => move |obj: &mut BandFilter, val: f64| {
                obj.set_range_start(nanometer!(val)).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid range-start value: {val}"));
                });
            },
            Self::RangeEnd => move |obj: &mut BandFilter, val: f64| {
                obj.set_range_end(nanometer!(val)).unwrap_or_else(|_| {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(&format!("Invalid range-end value: {val}"));
                });
            },
            Self::Resolution => move |obj: &mut BandFilter, val: f64| {
                obj.set_resolution(nanometer!(val)).unwrap_or_else(|_| {
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
