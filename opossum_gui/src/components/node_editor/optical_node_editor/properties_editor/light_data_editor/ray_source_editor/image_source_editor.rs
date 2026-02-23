use std::path::Path;

use crate::components::{
    logger::LogResultExt,
    node_editor::inputs::{
        InputData, InputParam, IntoInputData, IntoInputDataStrings, input_components::RowedInputs,
    },
};
use dioxus::prelude::*;
use opossum_core::{degree, joule};
use opossum_core::{
    meter,
    prelude::{ImageSrc, RayDataSource},
};
use strum::{EnumIter, IntoEnumIterator};
use uom::si::angle::degree;

#[component]
pub fn ImageSourceEditor(
    ray_data_builder_sig: ReadSignal<RayDataSource>,
    on_save: EventHandler<RayDataSource>,
) -> Element {
    match &*ray_data_builder_sig.read() {
        RayDataSource::Image(img_src) => {
            let inputs = get_image_source_input_params(img_src, on_save);
            rsx! {
                RowedInputs { inputs }
            }
        }
        _ => {
            rsx! {}
        }
    }
}

pub fn get_image_source_input_params(
    img_src: &ImageSrc,
    on_save: EventHandler<RayDataSource>,
) -> Vec<InputData> {
    let mut input_data = Vec::<InputData>::new();
    for enum_variant in ImageSrcParam::iter() {
        match enum_variant {
            ImageSrcParam::FPath => {
                input_data.push(
                    IntoInputData::<String, ImageSrc, RayDataSource>::to_input_data(
                        &enum_variant,
                        img_src.clone(),
                        on_save,
                    ),
                );
            }
            _ => input_data.push(
                IntoInputData::<f64, ImageSrc, RayDataSource>::to_input_data(
                    &enum_variant,
                    img_src.clone(),
                    on_save,
                ),
            ),
        }
    }
    input_data
}

#[derive(Clone, Copy, EnumIter, Eq, PartialEq)]
enum ImageSrcParam {
    FPath,
    PxlSize,
    Energy,
    Wavelength,
    ConeAngle,
}

impl From<ImageSrcParam> for InputParam {
    fn from(value: ImageSrcParam) -> Self {
        match value {
            ImageSrcParam::FPath => Self::FilePath("File:".into(), ".png".into()),
            ImageSrcParam::PxlSize => Self::SIUnit("Pixel size".into(), "m".into()),
            ImageSrcParam::Energy => Self::SIUnit("Energy".into(), "J".into()),
            ImageSrcParam::Wavelength => Self::SIUnit("Wavelength".into(), "m".into()),
            ImageSrcParam::ConeAngle => Self::SIUnit("Cone Angle".into(), "°".into()),
        }
    }
}

impl IntoInputDataStrings<ImageSrc> for ImageSrcParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::FPath => "File:",
            Self::PxlSize => "PxlSize",
            Self::Energy => "Energy",
            Self::Wavelength => "Wavelength",
            Self::ConeAngle => "ConeAngle",
        };

        format!("rayTypeImageSrc{id_str}Input")
    }
    fn create_value_string(&self, obj: &ImageSrc) -> String {
        match self {
            Self::FPath => obj
                .file_path()
                .file_name()
                .map_or("no file selected", |f| {
                    f.to_str().unwrap_or("no file selected")
                })
                .to_string(),
            Self::PxlSize => format!("{}", obj.pixel_size().value),
            Self::Energy => format!("{}", obj.energy().value),
            Self::Wavelength => format!("{}", obj.wavelength().value),
            Self::ConeAngle => format!("{}", obj.cone_angle().get::<degree>()),
        }
    }
}

impl IntoInputData<f64, ImageSrc, RayDataSource> for ImageSrcParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }
    fn setter_from_obj(&self) -> impl FnMut(&mut ImageSrc, f64) {
        match self {
            Self::PxlSize => move |obj: &mut ImageSrc, val: f64| {
                obj.set_pixel_size(meter!(val))
                    .log_err_with_context("validation failed in `set_pixel_size` of ImgSrc");
            },
            Self::Energy => move |obj: &mut ImageSrc, val: f64| {
                obj.set_energy(joule!(val))
                    .log_err_with_context("validation failed in `set_energy` of ImgSrc");
            },
            Self::Wavelength => move |obj: &mut ImageSrc, val: f64| {
                obj.set_wavelength(meter!(val))
                    .log_err_with_context("validation failed in `set_wavelength` of ImgSrc");
            },
            Self::ConeAngle => move |obj: &mut ImageSrc, val: f64| {
                obj.set_cone_angle(degree!(val))
                    .log_err_with_context("validation failed in `set_cone_angle` of ImgSrc");
            },
            Self::FPath => move |_: &mut ImageSrc, _: f64| {},
        }
    }
}

impl IntoInputData<String, ImageSrc, RayDataSource> for ImageSrcParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<String> {
        // 1. First, check if there is a text value (by using the rfd file selector)
        let value = e.value();
        if !value.is_empty() {
            return Some(value);
        }
        // 2. Fallback: Check for standard browser files (if used elsewhere)
        let files = e.files();
        if !files.is_empty() {
            return Some(files[0].name());
        }
        None
    }
    fn setter_from_obj(&self) -> impl FnMut(&mut ImageSrc, String) {
        if *self == Self::FPath {
            move |obj: &mut ImageSrc, val: String| {
                obj.set_file_path(Path::new(&val).to_path_buf())
                    .log_err_with_context("validation failed in `set_file_path` of ImgSrc");
            }
        } else {
            move |_: &mut ImageSrc, _: String| {}
        }
    }
}
