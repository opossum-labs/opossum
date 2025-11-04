use std::path::Path;

use crate::components::{
    logger::LogResultExt,
    node_editor::inputs::{
        InputData, InputParam, IntoInputData, IntoInputDataStrings, input_components::RowedInputs,
    },
};
use dioxus::prelude::*;
use opossum_core::prelude::{ImageSrc, RayDataBuilder};
use opossum_core::{degree, joule, micrometer, nanometer};
use strum::{EnumIter, IntoEnumIterator};
use uom::si::{
    angle::degree,
    energy::joule,
    length::{micrometer, nanometer},
};

#[component]
pub fn ImageSourceEditor(ray_data_builder_sig: Signal<RayDataBuilder>) -> Element {
    match &*ray_data_builder_sig.read() {
        RayDataBuilder::Image(img_src) => {
            let inputs = get_image_source_input_params(img_src, ray_data_builder_sig);
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
    ray_data_builder_sig: Signal<RayDataBuilder>,
) -> Vec<InputData> {
    let mut input_data = Vec::<InputData>::new();
    for enum_variant in ImageSrcParam::iter() {
        match enum_variant {
            ImageSrcParam::FPath => {
                input_data.push(
                    IntoInputData::<String, ImageSrc, RayDataBuilder>::to_input_data(
                        &enum_variant,
                        img_src.clone(),
                        ray_data_builder_sig,
                    ),
                );
            }
            _ => input_data.push(
                IntoInputData::<f64, ImageSrc, RayDataBuilder>::to_input_data(
                    &enum_variant,
                    img_src.clone(),
                    ray_data_builder_sig,
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
            ImageSrcParam::PxlSize => Self::F64("Pixel size in µm".into()),
            ImageSrcParam::Energy => Self::Energy("Energy in J".into()),
            ImageSrcParam::Wavelength => Self::Length("Wavelength in nm".into()),
            ImageSrcParam::ConeAngle => Self::Angle("Cone Angle in degrees".into()),
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
            Self::PxlSize => format!("{:.3e}", obj.pixel_size().get::<micrometer>()),
            Self::Energy => format!("{:.3e}", obj.energy().get::<joule>()),
            Self::Wavelength => format!("{:.3e}", obj.wavelength().get::<nanometer>()),
            Self::ConeAngle => format!("{:.3e}", obj.cone_angle().get::<degree>()),
        }
    }
}

impl IntoInputData<f64, ImageSrc, RayDataBuilder> for ImageSrcParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }
    fn setter_from_obj(&self) -> impl FnMut(&mut ImageSrc, f64) {
        match self {
            Self::PxlSize => move |obj: &mut ImageSrc, val: f64| {
                obj.set_pixel_size(micrometer!(val))
                    .log_err_with_context("validation failed in `set_pixel_size` of ImgSrc");
            },
            Self::Energy => move |obj: &mut ImageSrc, val: f64| {
                obj.set_energy(joule!(val))
                    .log_err_with_context("validation failed in `set_energy` of ImgSrc");
            },
            Self::Wavelength => move |obj: &mut ImageSrc, val: f64| {
                obj.set_wavelength(nanometer!(val))
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

impl IntoInputData<String, ImageSrc, RayDataBuilder> for ImageSrcParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<String> {
        if e.files().is_empty() {
            None
        } else {
            Some(e.files()[0].name().clone())
        }
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
